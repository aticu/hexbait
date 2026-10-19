//! Implements the types of the hexbait language.

use std::{collections::HashMap, fmt};

use crate::compile::{Span, ir::Symbol};

/// A type in the hexbait language.
#[derive(Clone)]
pub struct Ty {
    /// The kind of the type.
    pub kind: TyKind,
    /// The span pointing at the type definition.
    pub span: Span,
}

impl Ty {
    /// The name of the type.
    pub fn name(&self) -> TyName {
        self.kind.name()
    }

    /// Whether this type contains an error type recursively.
    pub fn contains_err(&self) -> bool {
        match &self.kind {
            TyKind::Error => true,
            TyKind::Indeterminate { .. }
            | TyKind::Bool
            | TyKind::Int { .. }
            | TyKind::EnumLit { .. }
            | TyKind::EnumVal { .. }
            | TyKind::FlagsVal { .. }
            | TyKind::Bytes => false,
            TyKind::Struct { struct_ty: ty } => {
                ty.fields.iter().any(|field| field.ty.contains_err())
            }
            TyKind::Array { item_ty } => item_ty.contains_err(),
        }
    }

    /// Whether the type supports comparisons with the other type.
    pub fn supports_comparisons_with(&self, other: &Ty) -> Option<ComparisonUnsupportedReason> {
        self.kind.supports_comparisons_with(&other.kind)
    }

    /// Joins the type of multiple branches into one type.
    pub fn join(branches: &[Ty], span: Span, last_branch_is_missing_else: bool) -> Ty {
        let kind = match &branches[0].kind {
            TyKind::Error | TyKind::Indeterminate { .. } => branches[0].kind.clone(),
            TyKind::Bool | TyKind::Bytes => Ty::join_primitive(branches, &branches[0].kind),
            TyKind::Int { .. } => Ty::join_int(branches),
            TyKind::EnumLit { .. } => Ty::join_enum_lit(branches),
            TyKind::EnumVal { .. } => Ty::join_enum_val(branches),
            TyKind::FlagsVal { .. } => Ty::join_flags_val(branches),
            TyKind::Struct { .. } => Ty::join_struct(branches, span, last_branch_is_missing_else),
            TyKind::Array { .. } => Ty::join_array(branches, span, last_branch_is_missing_else),
        };

        Ty { kind, span }
    }

    /// A generic type joining routine.
    ///
    /// This returns `Ok(())` only if `matches` returns `true` for all branches.
    fn join_generic(branches: &[Ty], mut matches: impl FnMut(&Ty) -> bool) -> Result<(), TyKind> {
        for ty in branches {
            if ty.contains_err() {
                return Err(TyKind::Error);
            }
            if matches!(ty.kind, TyKind::Indeterminate { .. }) {
                return Err(ty.kind.clone());
            }

            if !matches(ty) {
                return Err(TyKind::Indeterminate {
                    first_span: branches[0].span,
                    second_span: ty.span,
                });
            }
        }

        Ok(())
    }

    /// Joins the primitive type of the given kind into one type.
    fn join_primitive(branches: &[Ty], kind: &TyKind) -> TyKind {
        match Ty::join_generic(branches, |ty| {
            // comparing just the names is fine for primitives
            ty.name() == kind.name()
        }) {
            Ok(()) => kind.clone(),
            Err(kind) => kind,
        }
    }

    /// Joins multiple branches into an integer type.
    fn join_int(branches: &[Ty]) -> TyKind {
        let mut non_zero = true;
        let mut non_negative = true;

        match Ty::join_generic(branches, |ty| {
            if let TyKind::Int {
                non_zero: this_nz,
                non_negative: this_nn,
            } = &ty.kind
            {
                non_zero &= this_nz;
                non_negative &= this_nn;
                true
            } else {
                false
            }
        }) {
            Ok(()) => TyKind::Int {
                non_zero,
                non_negative,
            },
            Err(kind) => kind,
        }
    }

    /// Joins multiple branches into a struct type.
    fn join_struct(branches: &[Ty], span: Span, last_branch_is_missing_else: bool) -> TyKind {
        let mut fields = Vec::<FieldInfo>::new();

        struct FieldInfo {
            name: Symbol,
            first_seen: Span,
            first_missing: Option<Span>,
            inherited_conditional: Option<Availability>,
            tys: Vec<Ty>,
        }

        let mut is_first = true;
        match Ty::join_generic(branches, |ty| {
            let TyKind::Struct { struct_ty } = &ty.kind else {
                return false;
            };

            for field in &struct_ty.fields {
                let incoming_availability =
                    (!field.availability.is_guaranteed()).then(|| field.availability.clone());

                match fields.iter_mut().find(|f| f.name == field.name) {
                    Some(field_info) => {
                        if field_info.inherited_conditional.is_none() {
                            field_info.inherited_conditional = incoming_availability;
                        }
                        field_info.tys.push(field.ty.clone());
                    }
                    None => {
                        fields.push(FieldInfo {
                            name: field.name.clone(),
                            first_seen: ty.span,
                            first_missing: if is_first {
                                None
                            } else {
                                Some(branches[0].span)
                            },
                            inherited_conditional: incoming_availability,
                            tys: vec![field.ty.clone()],
                        });
                    }
                }
            }

            let mut missing = Vec::new();
            for (i, field_info) in fields.iter().enumerate() {
                if struct_ty.field(&field_info.name).is_none() {
                    missing.push(i);
                }
            }
            for idx in missing {
                fields[idx].first_missing.get_or_insert(ty.span);
            }

            is_first = false;
            true
        }) {
            Ok(()) => {
                let mut out_fields = Vec::new();

                for field_info in fields.into_iter() {
                    out_fields.push(FieldTy {
                        name: field_info.name,
                        ty: Ty::join(&field_info.tys, span, last_branch_is_missing_else),
                        availability: if let Some(first_conditional) =
                            field_info.inherited_conditional
                        {
                            first_conditional
                        } else if let Some(first_missing) = field_info.first_missing {
                            Availability::Conditional {
                                defined_at: field_info.first_seen,
                                undefined_at: first_missing,
                                undefined_is_missing_else: last_branch_is_missing_else
                                    && first_missing == branches.last().unwrap().span,
                            }
                        } else {
                            Availability::Guaranteed
                        },
                    });
                }

                TyKind::Struct {
                    struct_ty: StructTy { fields: out_fields },
                }
            }
            Err(kind) => kind,
        }
    }

    /// Joins multiple branches into an array type.
    fn join_array(branches: &[Ty], span: Span, last_branch_is_missing_else: bool) -> TyKind {
        let mut item_tys = Vec::new();

        match Ty::join_generic(branches, |ty| {
            if let TyKind::Array { item_ty } = &ty.kind {
                item_tys.push((**item_ty).clone());
                true
            } else {
                false
            }
        }) {
            Ok(()) => TyKind::Array {
                item_ty: Box::new(Ty::join(&item_tys, span, last_branch_is_missing_else)),
            },
            Err(kind) => kind,
        }
    }

    /// Joins multiple branches into an enum literal type.
    fn join_enum_lit(branches: &[Ty]) -> TyKind {
        let mut current_name = None;
        let mut unify_impossible = false;

        match Ty::join_generic(branches, |ty| {
            if let TyKind::EnumLit { known_name } = &ty.kind {
                if known_name.is_none() || unify_impossible {
                    unify_impossible = true;
                } else if current_name.is_none() {
                    current_name = known_name.clone();
                } else if known_name != &current_name {
                    unify_impossible = true;
                }
                true
            } else {
                false
            }
        }) {
            Ok(()) => TyKind::EnumLit {
                known_name: if unify_impossible { None } else { current_name },
            },
            Err(kind) => kind,
        }
    }

    /// Joins multiple branches into an enum value type.
    fn join_enum_val(branches: &[Ty]) -> TyKind {
        let mut variants = match &branches.first().unwrap().kind {
            TyKind::EnumVal { variants } => variants.clone(),
            _ => unreachable!(),
        };

        match Ty::join_generic(branches, |ty| {
            if let TyKind::EnumVal {
                variants: branch_variants,
            } = &ty.kind
            {
                for name in branch_variants.keys() {
                    if !variants.contains_key(name) {
                        variants.remove(name);
                    }
                }
                true
            } else {
                false
            }
        }) {
            Ok(()) => TyKind::EnumVal { variants },
            Err(kind) => kind,
        }
    }

    /// Joins multiple branches into an flags value type.
    fn join_flags_val(branches: &[Ty]) -> TyKind {
        let mut flags = match &branches.first().unwrap().kind {
            TyKind::FlagsVal { flags } => flags.clone(),
            _ => unreachable!(),
        };

        match Ty::join_generic(branches, |ty| {
            if let TyKind::FlagsVal {
                flags: branch_flags,
            } = &ty.kind
            {
                for name in branch_flags.keys() {
                    if !flags.contains_key(name) {
                        flags.remove(name);
                    }
                }
                true
            } else {
                false
            }
        }) {
            Ok(()) => TyKind::FlagsVal { flags },
            Err(kind) => kind,
        }
    }
}

/// The kind of a type in the hexbait language.
#[derive(Clone)]
pub enum TyKind {
    /// An invalid type produced by an already reported error.
    Error,
    /// The value could be of multiple different types.
    ///
    /// Thus no precise type information is available.
    Indeterminate {
        /// The span of the first diverging type.
        first_span: Span,
        /// The span of the second diverging type.
        second_span: Span,
    },
    /// A boolean value.
    Bool,
    /// An integer value.
    Int {
        /// Whether the integer is guaranteed to be non-zero.
        non_zero: bool,
        /// Whether the integer is guaranteed to be non-negative.
        non_negative: bool,
    },
    /// An `enum.field` literal.
    EnumLit {
        /// Whether the name of the literal is known.
        known_name: Option<String>,
    },
    /// An enum value.
    EnumVal {
        /// The names of the variants along with their definition spans.
        variants: HashMap<Symbol, Span>,
    },
    /// A flags value.
    FlagsVal {
        /// The names of the flags along with their definition spans.
        flags: HashMap<Symbol, Span>,
    },
    /// A bytes value.
    Bytes,
    /// A struct value.
    Struct {
        /// The struct type.
        struct_ty: StructTy,
    },
    /// An array value.
    Array {
        /// The type of the array items.
        item_ty: Box<Ty>,
    },
}

impl TyKind {
    /// The name of the type kind.
    pub fn name(&self) -> TyName {
        match self {
            TyKind::Error => unreachable!("error type should never be printed"),
            TyKind::Indeterminate { .. } => TyName::Indeterminate,
            TyKind::Bool => TyName::Bool,
            TyKind::Int { .. } => TyName::Int,
            TyKind::EnumLit { .. } => TyName::EnumLit,
            TyKind::EnumVal { .. } => TyName::EnumVal,
            TyKind::FlagsVal { .. } => TyName::FlagsVal,
            TyKind::Bytes => TyName::Bytes,
            TyKind::Struct { .. } => TyName::Struct,
            TyKind::Array { item_ty } => TyName::Array(Box::new(item_ty.name())),
        }
    }

    /// Whether the type supports comparisons with the other type.
    pub fn supports_comparisons_with(&self, other: &TyKind) -> Option<ComparisonUnsupportedReason> {
        #[expect(
            unreachable_patterns,
            reason = "to make the last code path explicit and unify it with the others"
        )]
        match (self, other) {
            (TyKind::Error, _)
            | (_, TyKind::Error)
            | (TyKind::Bool, TyKind::Bool)
            | (TyKind::Int { .. }, TyKind::Int { .. })
            | (TyKind::EnumVal { .. }, TyKind::Int { .. })
            | (TyKind::Int { .. }, TyKind::EnumVal { .. })
            | (TyKind::FlagsVal { .. }, TyKind::Int { .. })
            | (TyKind::Int { .. }, TyKind::FlagsVal { .. })
            | (TyKind::Bytes, TyKind::Bytes) => None,
            (TyKind::EnumVal { variants }, TyKind::EnumLit { known_name })
            | (TyKind::EnumLit { known_name }, TyKind::EnumVal { variants }) => {
                if let Some(known_name) = known_name {
                    if variants.contains_key(&Symbol::from(&**known_name)) {
                        None
                    } else {
                        Some(ComparisonUnsupportedReason::EnumVariantUnknown)
                    }
                } else {
                    None
                }
            }
            (TyKind::EnumLit { .. }, _) | (_, TyKind::EnumLit { .. }) => {
                Some(ComparisonUnsupportedReason::EnumLitsIncomparable)
            }
            (TyKind::EnumVal { .. }, _) | (_, TyKind::EnumVal { .. }) => {
                Some(ComparisonUnsupportedReason::EnumValOnlyIntAndLit)
            }
            (TyKind::FlagsVal { .. }, _) | (_, TyKind::FlagsVal { .. }) => {
                Some(ComparisonUnsupportedReason::FlagsValOnlyInt)
            }
            (TyKind::Bool, _)
            | (_, TyKind::Bool)
            | (TyKind::Int { .. }, _)
            | (_, TyKind::Int { .. })
            | (TyKind::Bytes, _)
            | (_, TyKind::Bytes) => Some(ComparisonUnsupportedReason::DifferentTypes),
            (TyKind::Indeterminate { .. }, _)
            | (_, TyKind::Indeterminate { .. })
            | (TyKind::Struct { .. }, _)
            | (_, TyKind::Struct { .. })
            | (TyKind::Array { .. }, _)
            | (_, TyKind::Array { .. }) => Some(ComparisonUnsupportedReason::IncomparableType),
        }
    }
}

/// The type of a struct.
#[derive(Clone)]
pub struct StructTy {
    /// The fields of the struct.
    fields: Vec<FieldTy>,
}

impl StructTy {
    /// Returns the type of an empty struct.
    pub fn empty() -> StructTy {
        StructTy { fields: Vec::new() }
    }

    /// Adds the given field to the struct type.
    pub fn add_field(&mut self, field: FieldTy) {
        self.fields.push(field);
    }

    /// Returns the field with the given name.
    pub fn field(&self, name: &Symbol) -> Option<&FieldTy> {
        self.fields.iter().find(|field| &field.name == name)
    }
}

/// The type of a struct field.
#[derive(Clone)]
pub struct FieldTy {
    /// The name of the field.
    pub name: Symbol,
    /// The type of the field.
    pub ty: Ty,
    /// Where this field is available.
    pub availability: Availability,
}

/// The availability of a field.
#[derive(Clone)]
pub enum Availability {
    /// The field is guaranteed to be available.
    Guaranteed,
    /// The field is missing in at least one branch.
    Conditional {
        /// The span of the first branch where the field is defined.
        defined_at: Span,
        /// The span that describes why this field is unavailable.
        undefined_at: Span,
        /// Whether the undefined branch is a missing `else` branch.
        undefined_is_missing_else: bool,
    },
}

impl Availability {
    /// Whether the availability of the field is guaranteed.
    pub fn is_guaranteed(&self) -> bool {
        match self {
            Availability::Guaranteed => true,
            Availability::Conditional { .. } => false,
        }
    }
}

/// The name of a type.
#[derive(PartialEq, Eq)]
pub enum TyName {
    /// The indeterminate type.
    Indeterminate,
    /// The boolean type.
    Bool,
    /// The integer type.
    Int,
    /// The bytes type.
    Bytes,
    /// A struct type.
    Struct,
    /// An array type.
    Array(Box<TyName>),
    /// An enum literal type.
    EnumLit,
    /// An enum type.
    EnumVal,
    /// A flags type.
    FlagsVal,
}

impl fmt::Display for TyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TyName::Indeterminate => write!(f, "indeterminate type"),
            TyName::Bool => write!(f, "bool"),
            TyName::Int => write!(f, "int"),
            TyName::Bytes => write!(f, "bytes"),
            TyName::Struct => write!(f, "{{ .. }}"),
            TyName::Array(ty_name) => write!(f, "[{ty_name}]"),
            TyName::EnumLit => write!(f, "enum literal"),
            TyName::EnumVal => write!(f, "enum value"),
            TyName::FlagsVal => write!(f, "flags value"),
        }
    }
}

/// The reason why types cannot be compared.
pub enum ComparisonUnsupportedReason {
    /// The types are different and not directly comparable.
    DifferentTypes,
    /// One of the types supports no comparisons in general.
    IncomparableType,
    /// Enum literals are only comparable against enum values.
    EnumLitsIncomparable,
    /// Enum values can only be compared to integers and enum literals.
    EnumValOnlyIntAndLit,
    /// Flags values can only be compared to integers.
    FlagsValOnlyInt,
    /// The enum variant is unknown.
    EnumVariantUnknown,
}

impl fmt::Display for ComparisonUnsupportedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "help: ")?;
        match self {
            ComparisonUnsupportedReason::DifferentTypes => write!(f, "the types differ"),
            ComparisonUnsupportedReason::IncomparableType => {
                write!(f, "at least one of the types is incomparable")
            }
            ComparisonUnsupportedReason::EnumLitsIncomparable => {
                write!(f, "enum literals can only be compared to enum values")
            }
            ComparisonUnsupportedReason::EnumValOnlyIntAndLit => {
                write!(
                    f,
                    "enum values can only be compared to enum literals and integers"
                )
            }
            ComparisonUnsupportedReason::FlagsValOnlyInt => {
                write!(f, "flags values can only be compared to integers")
            }
            ComparisonUnsupportedReason::EnumVariantUnknown => {
                write!(f, "enum variant is unknown")
            }
        }
    }
}
