//! Implements the types of the hexbait language.

use std::fmt;

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
            TyKind::Indeterminate { .. } | TyKind::Bool | TyKind::Int { .. } | TyKind::Bytes => {
                false
            }
            TyKind::Struct { struct_ty: ty } => {
                ty.fields.iter().any(|field| field.ty.contains_err())
            }
            TyKind::Array { item_ty } => item_ty.contains_err(),
        }
    }

    /// Whether this type is logically equivalent to `other`.
    pub fn unifies(&self, other: &Ty) -> bool {
        self.kind.unifies(&other.kind)
    }

    /// Whether the type supports comparisons.
    pub fn supports_comparisons(&self) -> bool {
        match &self.kind {
            TyKind::Error | TyKind::Bool | TyKind::Int { .. } | TyKind::Bytes => true,
            TyKind::Indeterminate { .. } | TyKind::Struct { .. } | TyKind::Array { .. } => false,
        }
    }

    /// Joins the type of multiple branches into one type.
    pub fn join(branches: &[Ty], span: Span, last_branch_is_missing_else: bool) -> Ty {
        let kind = match &branches[0].kind {
            TyKind::Error | TyKind::Indeterminate { .. } => branches[0].kind.clone(),
            TyKind::Bool | TyKind::Bytes => Ty::join_primitive(branches, &branches[0].kind),
            TyKind::Int { .. } => Ty::join_int(branches),
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
            TyKind::Bytes => TyName::Bytes,
            TyKind::Struct { .. } => TyName::Struct,
            TyKind::Array { item_ty } => TyName::Array(Box::new(item_ty.name())),
        }
    }

    /// Whether this type is logically equivalent to `other`.
    pub fn unifies(&self, other: &TyKind) -> bool {
        match (self, other) {
            // error unifies with everything
            (TyKind::Error, _) | (_, TyKind::Error) => true,

            // indeterminate unifies with nothing
            (TyKind::Indeterminate { .. }, _) | (_, TyKind::Indeterminate { .. }) => false,

            // primitives only unify with themselves
            (TyKind::Bool, TyKind::Bool) => true,
            (TyKind::Bool, _) | (_, TyKind::Bool) => false,
            (TyKind::Int { .. }, TyKind::Int { .. }) => true,
            (TyKind::Int { .. }, _) | (_, TyKind::Int { .. }) => false,
            (TyKind::Bytes, TyKind::Bytes) => true,
            (TyKind::Bytes, _) | (_, TyKind::Bytes) => false,

            // structs unify with themselves if all fields match
            (TyKind::Struct { struct_ty: left }, TyKind::Struct { struct_ty: right }) => {
                left.unifies(right)
            }
            (TyKind::Struct { .. }, _) | (_, TyKind::Struct { .. }) => false,

            // array unify with themselves if the elements unify
            (TyKind::Array { item_ty: left_ty }, TyKind::Array { item_ty: right_ty }) => {
                left_ty.unifies(right_ty)
            }
            #[expect(
                unreachable_patterns,
                reason = "to make the last code path explicit and unify it with the others"
            )]
            (TyKind::Array { .. }, _) | (_, TyKind::Array { .. }) => false,
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

    /// Whether the structs are logically equivalent.
    fn unifies(&self, other: &StructTy) -> bool {
        if self.fields.len() != other.fields.len() {
            return false;
        }

        self.fields.iter().all(|field| {
            let Some(other_field) = other.field(&field.name) else {
                return false;
            };

            if !field.availability.is_guaranteed() || !other_field.availability.is_guaranteed() {
                // we cannot guarantee anything about non-guaranteed fields, so they cannot even unify with themselves
                return false;
            }

            field.ty.unifies(&other_field.ty)
        })
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
        }
    }
}
