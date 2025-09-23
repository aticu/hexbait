//! Implements the parsing capabilities of hexbait.

use language::ast::{BinOp, Expr, ExprKind, Node, NodeKind, UnOp};

pub mod eval;
pub mod language;

// TODO: implement a frontend for parsing `Node`s from text
// TODO: implement display options (enum that name certain values)
// TODO: implement bitwise fields
// TODO: implement custom data streams
// TODO: implement classification of parsed values (offset, integer?, string?)
// TODO: improve display of the parsed values in the GUI
// TODO: figure out a way to cleverly incorporate colors
// TODO: implement jumping to offsets in the hexview

// TODO: remove when this can be parsed from a text file
pub fn tmp_pe_file() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "mz_header".into(),
                    Node {
                        kind: NodeKind::Struct {
                            nodes: vec![
                                (
                                    "mz_magic".into(),
                                    Node {
                                        kind: NodeKind::FixedBytes {
                                            expected: Expr {
                                                kind: ExprKind::ConstantBytes {
                                                    value: b"MZ".into(),
                                                },
                                            },
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "new_exe_header".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: Some(Expr {
                                            kind: ExprKind::ConstantInt { value: 60.into() },
                                        }),
                                    },
                                ),
                            ],
                        },
                        offset: None,
                    },
                ),
                (
                    "pe_header".into(),
                    Node {
                        kind: NodeKind::Struct {
                            nodes: vec![
                                (
                                    "pe_magic".into(),
                                    Node {
                                        kind: NodeKind::FixedBytes {
                                            expected: Expr {
                                                kind: ExprKind::ConstantBytes {
                                                    value: b"PE\0\0".into(),
                                                },
                                            },
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "machine".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "num_of_sections".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "time_date_stamp".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "pointer_to_symbol_table".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "num_of_symbols".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "size_of_optional_header".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "characteristics".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                            ],
                        },
                        offset: Some(Expr {
                            kind: ExprKind::FieldAccess {
                                field_holder: Box::new(Expr {
                                    kind: ExprKind::VariableUse {
                                        var: "mz_header".into(),
                                    },
                                }),
                                field: "new_exe_header".into(),
                            },
                        }),
                    },
                ),
                (
                    "optional_header".into(),
                    Node {
                        kind: NodeKind::Struct {
                            nodes: vec![
                                (
                                    "optional_magic".into(),
                                    Node {
                                        kind: NodeKind::FixedBytes {
                                            expected: Expr {
                                                kind: ExprKind::ConstantBytes {
                                                    value: b"\x0b\x01".into(),
                                                },
                                            },
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "major_linker_version".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 8,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "minor_linker_version".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 8,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "size_of_code".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "size_of_initialized_data".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "size_of_uninitialized_data".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "addr_of_entry_point".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "base_of_code".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "base_of_data".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                            ],
                        },
                        offset: None,
                    },
                ),
                (
                    "sections".into(),
                    Node {
                        kind: NodeKind::RepeatCount {
                            node: Box::new(Node {
                                kind: NodeKind::Struct {
                                    nodes: vec![
                                        (
                                            "section_name".into(),
                                            Node {
                                                kind: NodeKind::FixedLength {
                                                    length: Expr {
                                                        kind: ExprKind::ConstantInt {
                                                            value: 8.into(),
                                                        },
                                                    },
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "virtual_size".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "virtual_address".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "size_of_raw_data".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "pointer_to_raw_data".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "pointer_to_relocations".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "pointer_to_line_numbers".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "number_of_relocations".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "number_of_line_numbers".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "characteristics".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "content".into(),
                                            Node {
                                                kind: NodeKind::Elsewhere {
                                                    node: Box::new(Node {
                                                        kind: NodeKind::FixedLength {
                                                            length: Expr {
                                                                kind: ExprKind::VariableUse {
                                                                    var: "size_of_raw_data".into(),
                                                                },
                                                            },
                                                        },
                                                        offset: Some(Expr {
                                                            kind: ExprKind::VariableUse {
                                                                var: "pointer_to_raw_data".into(),
                                                            },
                                                        }),
                                                    }),
                                                },
                                                offset: None,
                                            }
                                        ),
                                    ],
                                },
                                offset: None,
                            }),
                            count: Expr {
                                kind: ExprKind::FieldAccess {
                                    field_holder: Box::new(Expr {
                                        kind: ExprKind::VariableUse {
                                            var: "pe_header".into(),
                                        },
                                    }),
                                    field: "num_of_sections".into(),
                                },
                            },
                        },
                        offset: Some(Expr {
                            kind: ExprKind::BinOp {
                                left: Box::new(Expr {
                                    kind: ExprKind::FieldAccess {
                                        field_holder: Box::new(Expr {
                                            kind: ExprKind::VariableUse {
                                                var: "pe_header".into(),
                                            },
                                        }),
                                        field: "size_of_optional_header".into(),
                                    },
                                }),
                                right: Box::new(Expr {
                                    kind: ExprKind::BinOp {
                                        left: Box::new(Expr {
                                            kind: ExprKind::ConstantInt { value: 24.into() },
                                        }),
                                        right: Box::new(Expr {
                                            kind: ExprKind::FieldAccess {
                                                field_holder: Box::new(Expr {
                                                    kind: ExprKind::VariableUse {
                                                        var: "mz_header".into(),
                                                    },
                                                }),
                                                field: "new_exe_header".into(),
                                            },
                                        }),
                                        op: BinOp::Add,
                                    },
                                }),
                                op: BinOp::Add,
                            },
                        }),
                    },
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_mft_entry() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "header".into(),
                    Node {
                        kind: NodeKind::Struct {
                            nodes: vec![
                                (
                                    "magic".into(),
                                    Node {
                                        kind: NodeKind::FixedBytes {
                                            expected: Expr {
                                                kind: ExprKind::ConstantBytes {
                                                    value: b"FILE".into(),
                                                },
                                            },
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "fixup_value_offset".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "num_fixup_values".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "log_sequence_number".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 64,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "sequence_number".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "reference_count".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "first_attribute".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "entry_flags".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "used_entry_size".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "total_entry_size".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "file_reference".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 64,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                                (
                                    "first_attribute_identifier".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                ),
                            ],
                        },
                        offset: None,
                    },
                ),
                (
                    "attributes".into(),
                    Node {
                        kind: NodeKind::RepeatWhile {
                            node: Box::new(Node {
                                kind: NodeKind::Struct {
                                    nodes: vec![
                                        (
                                            "type".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "size".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "nonresident".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 8,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "name_size".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 8,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "name_offset".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "attribute_data_flags".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "attribute_identifier".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "data".into(),
                                            Node {
                                                kind: NodeKind::Switch {
                                                    scrutinee: Expr {
                                                        kind: ExprKind::VariableUse {
                                                            var: "nonresident".into(),
                                                        },
                                                    },
                                                    branches: vec![(
                                                        Expr {
                                                            kind: ExprKind::ConstantInt {
                                                                value: 0.into(),
                                                            },
                                                        },
                                                        Node {
                                                            kind: NodeKind::Struct {
                                                                nodes: vec![
                                                                    (
                                                                        "data_size".into(),
                                                                        Node {
                                                                            kind:
                                                                                NodeKind::Integer {
                                                                                    bit_width: 32,
                                                                                    signed: false,
                                                                                },
                                                                            offset: None,
                                                                        },
                                                                    ),
                                                                    (
                                                                        "data_offset".into(),
                                                                        Node {
                                                                            kind:
                                                                                NodeKind::Integer {
                                                                                    bit_width: 16,
                                                                                    signed: false,
                                                                                },
                                                                            offset: None,
                                                                        },
                                                                    ),
                                                                    (
                                                                        "indexed_flag".into(),
                                                                        Node {
                                                                            kind:
                                                                                NodeKind::Integer {
                                                                                    bit_width: 8,
                                                                                    signed: false,
                                                                                },
                                                                            offset: None,
                                                                        },
                                                                    ),
                                                                    (
                                                                        "padding".into(),
                                                                        Node {
                                                                            kind: NodeKind::FixedBytes {
                                                                                expected: Expr {
                                                                                    kind: ExprKind::ConstantBytes {
                                                                                        value: vec![0x00],
                                                                                    }
                                                                                }
                                                                            },
                                                                            offset: None,
                                                                        },
                                                                    ),
                                                                    (
                                                                        "content".into(),
                                                                        Node {
                                                                            kind: NodeKind::Switch {
                                                                                scrutinee: Expr {
                                                                                    kind: ExprKind::FieldAccess {
                                                                                        field_holder: Box::new(Expr {
                                                                                            kind: ExprKind::Parent
                                                                                        }),
                                                                                        field: "type".into(),
                                                                                    }
                                                                                },
                                                                                branches: vec![
                                                                                    (
                                                                                        Expr {
                                                                                            kind: ExprKind::ConstantInt {
                                                                                                value: 0x10.into()
                                                                                            },
                                                                                        },
                                                                                        Node {
                                                                                            kind: NodeKind::NamedNode {
                                                                                                name: "mft_attr_stdinfo".into(),
                                                                                            },
                                                                                            offset: None,
                                                                                        },
                                                                                    ),
                                                                                    (
                                                                                        Expr {
                                                                                            kind: ExprKind::ConstantInt {
                                                                                                value: 0x30.into()
                                                                                            },
                                                                                        },
                                                                                        Node {
                                                                                            kind: NodeKind::Struct {
                                                                                                nodes: vec![
                                                                                                    (
                                                                                                        "attribute_content".into(),
                                                                                                        Node {
                                                                                                            kind: NodeKind::NamedNode {
                                                                                                                name: "mft_attr_filename".into(),
                                                                                                            },
                                                                                                            offset: None,
                                                                                                        },
                                                                                                    ),
                                                                                                    (
                                                                                                        "unused".into(),
                                                                                                        Node {
                                                                                                            kind: NodeKind::FixedLength {
                                                                                                                length: Expr {
                                                                                                                    kind: ExprKind::BinOp {
                                                                                                                        left: Box::new(Expr {
                                                                                                                            kind: ExprKind::FieldAccess {
                                                                                                                                field_holder: Box::new(Expr {
                                                                                                                                    kind: ExprKind::Parent,
                                                                                                                                }),
                                                                                                                                field: "data_size".into(),
                                                                                                                            },
                                                                                                                        }),
                                                                                                                        right: Box::new(Expr {
                                                                                                                            kind: ExprKind::BinOp {
                                                                                                                                left: Box::new(Expr {
                                                                                                                                    kind: ExprKind::BinOp {
                                                                                                                                        left: Box::new(Expr {
                                                                                                                                            kind: ExprKind::FieldAccess {
                                                                                                                                                field_holder: Box::new(Expr {
                                                                                                                                                    kind: ExprKind::VariableUse {
                                                                                                                                                        var: "attribute_content".into(),
                                                                                                                                                    }
                                                                                                                                                }),
                                                                                                                                                field: "name_string_size".into(),
                                                                                                                                            },
                                                                                                                                        }),
                                                                                                                                        right: Box::new(Expr {
                                                                                                                                            kind: ExprKind::ConstantInt {
                                                                                                                                                value: 2.into(),
                                                                                                                                            },
                                                                                                                                        }),
                                                                                                                                        op: BinOp::Mul,
                                                                                                                                    },
                                                                                                                                }),
                                                                                                                                right: Box::new(Expr {
                                                                                                                                    kind: ExprKind::ConstantInt { value: 66.into() },
                                                                                                                                }),
                                                                                                                                op: BinOp::Add,
                                                                                                                            },
                                                                                                                        }),
                                                                                                                        op: BinOp::Sub,
                                                                                                                    },
                                                                                                                },
                                                                                                            },
                                                                                                            offset: None,
                                                                                                        },
                                                                                                    ),
                                                                                                ],
                                                                                            },
                                                                                            offset: None,
                                                                                        }
                                                                                    ),
                                                                                    (
                                                                                        Expr {
                                                                                            kind: ExprKind::ConstantInt {
                                                                                                value: 0x90.into()
                                                                                            },
                                                                                        },
                                                                                        Node {
                                                                                            kind: NodeKind::NamedNode {
                                                                                                name: "mft_index_root".into(),
                                                                                            },
                                                                                            offset: None,
                                                                                        },
                                                                                    ),
                                                                                ],
                                                                                default: Box::new(Node {
                                                                                    kind: NodeKind::FixedLength {
                                                                                        length: Expr {
                                                                                            kind: ExprKind::VariableUse {
                                                                                                var: "data_size".into()
                                                                                            }
                                                                                        }
                                                                                    },
                                                                                    offset: None,
                                                                                }),
                                                                            },
                                                                            offset: Some(
                                                                                Expr {
                                                                                    kind: ExprKind::BinOp {
                                                                                        left: Box::new(Expr {
                                                                                            kind: ExprKind::Offset,
                                                                                        }),
                                                                                        right: Box::new(Expr {
                                                                                            kind: ExprKind::BinOp {
                                                                                                left: Box::new(Expr {
                                                                                                    kind: ExprKind::VariableUse {
                                                                                                        var: "data_offset".into()
                                                                                                    }
                                                                                                }),
                                                                                                right: Box::new(Expr {
                                                                                                    kind: ExprKind::ConstantInt {
                                                                                                        value: 24.into()
                                                                                                    }
                                                                                                }),
                                                                                                op: BinOp::Sub
                                                                                            }
                                                                                        }),
                                                                                        op: BinOp::Add
                                                                                    }
                                                                                }
                                                                            ),
                                                                        },
                                                                    ),
                                                                    (
                                                                        "unused".into(),
                                                                        Node {
                                                                            kind: NodeKind::FixedLength {
                                                                                length: Expr {
                                                                                    kind: ExprKind::BinOp {
                                                                                        left: Box::new(Expr {
                                                                                            kind: ExprKind::FieldAccess{
                                                                                                field_holder: Box::new(Expr {
                                                                                                    kind: ExprKind::Parent
                                                                                                }),
                                                                                                field: "size".into(),
                                                                                            }
                                                                                        }),
                                                                                        right: Box::new(Expr {
                                                                                            kind: ExprKind::BinOp {
                                                                                                left: Box::new(Expr {
                                                                                                    kind: ExprKind::VariableUse {
                                                                                                        var: "data_offset".into()
                                                                                                    }
                                                                                                }),
                                                                                                right: Box::new(Expr {
                                                                                                    kind: ExprKind::VariableUse {
                                                                                                        var: "data_size".into()
                                                                                                    }
                                                                                                }),
                                                                                                op: BinOp::Add,
                                                                                            }
                                                                                        }),
                                                                                        op: BinOp::Sub
                                                                                    }
                                                                                }
                                                                            },
                                                                            offset: None,
                                                                        },
                                                                    ),
                                                                ],
                                                            },
                                                            offset: None,
                                                        },
                                                    )],
                                                    default: Box::new(Node {
                                                        kind: NodeKind::Struct {
                                                            nodes: vec![
                                                                (
                                                                    "first_virtual_cluster_number".into(),
                                                                    Node {
                                                                        kind: NodeKind::Integer {
                                                                            bit_width: 64,
                                                                            signed: false
                                                                        },
                                                                        offset: None,
                                                                    }
                                                                ),
                                                                (
                                                                    "last_virtual_cluster_number".into(),
                                                                    Node {
                                                                        kind: NodeKind::Integer {
                                                                            bit_width: 64,
                                                                            signed: false
                                                                        },
                                                                        offset: None,
                                                                    }
                                                                ),
                                                                (
                                                                    "data_runs_offset".into(),
                                                                    Node {
                                                                        kind: NodeKind::Integer {
                                                                            bit_width: 16,
                                                                            signed: false
                                                                        },
                                                                        offset: None,
                                                                    }
                                                                ),
                                                                (
                                                                    "compression_unit_size".into(),
                                                                    Node {
                                                                        kind: NodeKind::Integer {
                                                                            bit_width: 16,
                                                                            signed: false
                                                                        },
                                                                        offset: None,
                                                                    }
                                                                ),
                                                                (
                                                                    "padding".into(),
                                                                    Node {
                                                                        kind: NodeKind::FixedBytes {
                                                                            expected: Expr {
                                                                                kind: ExprKind::ConstantBytes {
                                                                                    value: vec![0x00, 0x00, 0x00, 0x00]
                                                                                }
                                                                            }
                                                                        },
                                                                        offset: None,
                                                                    }
                                                                ),
                                                                (
                                                                    "allocated_data_size".into(),
                                                                    Node {
                                                                        kind: NodeKind::Integer {
                                                                            bit_width: 64,
                                                                            signed: false
                                                                        },
                                                                        offset: None,
                                                                    }
                                                                ),
                                                                (
                                                                    "data_size".into(),
                                                                    Node {
                                                                        kind: NodeKind::Integer {
                                                                            bit_width: 64,
                                                                            signed: false
                                                                        },
                                                                        offset: None,
                                                                    }
                                                                ),
                                                                (
                                                                    "valid_data_size".into(),
                                                                    Node {
                                                                        kind: NodeKind::Integer {
                                                                            bit_width: 64,
                                                                            signed: false
                                                                        },
                                                                        offset: None,
                                                                    }
                                                                ),
                                                                (
                                                                    "total_allocated_size".into(),
                                                                    Node {
                                                                        kind: NodeKind::Integer {
                                                                            bit_width: 64,
                                                                            signed: false
                                                                        },
                                                                        offset: None,
                                                                    }
                                                                ),
                                                                (
                                                                    "rest".into(),
                                                                    Node {
                                                                        kind: NodeKind::FixedLength {
                                                                            length: Expr {
                                                                                kind: ExprKind::BinOp {
                                                                                    left: Box::new(Expr {
                                                                                        kind: ExprKind::FieldAccess{
                                                                                            field_holder: Box::new(Expr {
                                                                                                kind: ExprKind::Parent
                                                                                            }),
                                                                                            field: "size".into(),
                                                                                        }
                                                                                    }),
                                                                                    right: Box::new(Expr {
                                                                                        kind: ExprKind::ConstantInt {
                                                                                            value: 72.into()
                                                                                        }
                                                                                    }),
                                                                                    op: BinOp::Sub
                                                                                }
                                                                            },
                                                                        },
                                                                        offset: None,
                                                                    }
                                                                ),
                                                            ],
                                                        },
                                                        offset: None,
                                                    }),
                                                },
                                                offset: None,
                                            },
                                        ),
                                    ],
                                },
                                offset: None,
                            }),
                            condition: Expr {
                                kind: ExprKind::UnOp {
                                    operand: Box::new(Expr {
                                        kind: ExprKind::BinOp {
                                            left: Box::new(Expr {
                                                kind: ExprKind::ParseAt {
                                                    node: Box::new(Node {
                                                        kind: NodeKind::FixedLength {
                                                            length: Expr {
                                                                kind: ExprKind::ConstantInt {
                                                                    value: 4.into(),
                                                                },
                                                            },
                                                        },
                                                        offset: None,
                                                    }),
                                                },
                                            }),
                                            right: Box::new(Expr {
                                                kind: ExprKind::ConstantBytes {
                                                    value: vec![0xff, 0xff, 0xff, 0xff],
                                                },
                                            }),
                                            op: BinOp::Eq,
                                        },
                                    }),
                                    op: UnOp::Not,
                                },
                            },
                        },
                        offset: Some(Expr {
                            kind: ExprKind::FieldAccess {
                                field_holder: Box::new(Expr {
                                    kind: ExprKind::VariableUse {
                                        var: "header".into(),
                                    },
                                }),
                                field: "first_attribute".into(),
                            },
                        }),
                    },
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_mft_standard_information() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "creation_time".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "modification_time".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "changed_time".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "access_time".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "file_attribute_flags".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "maximum_number_of_versions".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "version_number".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "class_identifier".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "owner_identifier".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "security_descriptor_identifier".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "quota_charged".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "update_sequence_number".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_mft_file_name() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "parent_file_reference".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "creation_time".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "modification_time".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "changed_time".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "access_time".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "allocated_file_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "file_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "file_attribute_flags".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "extended_data".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "name_string_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 8,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "namespace".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 8,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "name".into(),
                    Node {
                        kind: NodeKind::FixedLength {
                            length: Expr {
                                kind: ExprKind::BinOp {
                                    left: Box::new(Expr {
                                        kind: ExprKind::VariableUse {
                                            var: "name_string_size".into(),
                                        },
                                    }),
                                    right: Box::new(Expr {
                                        kind: ExprKind::ConstantInt { value: 2.into() },
                                    }),
                                    op: BinOp::Mul,
                                },
                            },
                        },
                        offset: None,
                    },
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_mft_index_root() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "attribute_type".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "collation_type".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "index_entry_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "index_entry_number_of_cluster_blocks".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "index_node_header".into(),
                    Node {
                        kind: NodeKind::NamedNode {
                            name: "mft_index_node_header".into(),
                        },
                        offset: None,
                    },
                ),
                (
                    "unused".into(),
                    Node {
                        kind: NodeKind::FixedLength {
                            length: Expr {
                                kind: ExprKind::BinOp {
                                    left: Box::new(Expr {
                                        kind: ExprKind::FieldAccess {
                                            field_holder: Box::new(Expr {
                                                kind: ExprKind::Parent,
                                            }),
                                            field: "data_size".into(),
                                        },
                                    }),
                                    right: Box::new(Expr {
                                        kind: ExprKind::ConstantInt { value: 32.into() },
                                    }),
                                    op: BinOp::Sub,
                                },
                            },
                        },
                        offset: None,
                    },
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_mft_index_node_header() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "index_values_offset".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "index_node_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "allocated_index_node_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "index_node_flags".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_mft_index_entry() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "signature".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"INDX".into()
                                }
                            }
                        },
                        offset: None,
                    }
                ),
                (
                    "fixup_values_offset".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false
                        },
                        offset: None,
                    }
                ),
                (
                    "num_of_fixup_values".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false
                        },
                        offset: None,
                    }
                ),
                (
                    "metadata_transaction_journal_sequence_number".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false
                        },
                        offset: None,
                    }
                ),
                (
                    "virtual_cluster_number_of_index_entry".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false
                        },
                        offset: None,
                    }
                ),
                (
                    "index_node_header".into(),
                    Node {
                        kind: NodeKind::NamedNode {
                            name: "mft_index_node_header".into(),
                        },
                        offset: None,
                    }
                ),
                (
                    "values".into(),
                    Node {
                        kind: NodeKind::RepeatWhile {
                            node: Box::new(Node {
                                kind: NodeKind::Struct {
                                    nodes: vec![
                                        (
                                            "file_reference".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 64,
                                                    signed: false
                                                },
                                                offset: None,
                                            }
                                        ),
                                        (
                                            "index_value_size".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false
                                                },
                                                offset: None,
                                            }
                                        ),
                                        (
                                            "index_key_data_size".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false
                                                },
                                                offset: None,
                                            }
                                        ),
                                        (
                                            "index_value_flags".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false
                                                },
                                                offset: None,
                                            }
                                        ),
                                        (
                                            "index_key_data".into(),
                                            Node {
                                                kind: NodeKind::ParseIf {
                                                    condition: Expr {
                                                        kind: ExprKind::BinOp {
                                                            left: Box::new(Expr {
                                                                kind: ExprKind::VariableUse {
                                                                    var: "index_key_data_size".into()
                                                                }
                                                            }),
                                                            right: Box::new(Expr {
                                                                kind: ExprKind::ConstantInt {
                                                                    value: 66.into()
                                                                }
                                                            }),
                                                            op: BinOp::Geq
                                                        }
                                                    },
                                                    true_node: Box::new(Node {
                                                        kind: NodeKind::NamedNode {
                                                            name: "mft_attr_filename".into(),
                                                        },
                                                        offset: None
                                                    }),
                                                    false_node: Box::new(Node {
                                                        kind: NodeKind::FixedLength {
                                                            length: Expr {
                                                                kind: ExprKind::VariableUse {
                                                                    var: "index_key_data_size".into()
                                                                }
                                                            }
                                                        },
                                                        offset: None,
                                                    })
                                                },
                                                offset: None,
                                            }
                                        ),
                                        (
                                            "alignment_padding".into(),
                                            Node {
                                                kind: NodeKind::FixedLength {
                                                    length: Expr {
                                                        kind: ExprKind::If {
                                                            condition: Box::new(Expr {
                                                                kind: ExprKind::BinOp {
                                                                    left: Box::new(Expr {
                                                                        kind: ExprKind::ConstantInt {
                                                                            value: 0.into()
                                                                        }
                                                                    }),
                                                                    right: Box::new(Expr {
                                                                        kind: ExprKind::BinOp {
                                                                            left: Box::new(Expr {
                                                                                kind: ExprKind::Offset
                                                                            }),
                                                                            right: Box::new(Expr {
                                                                                kind: ExprKind::ConstantInt {
                                                                                    value: 8.into()
                                                                                }
                                                                            }),
                                                                            op: BinOp::Mod
                                                                        }
                                                                    }),
                                                                    op: BinOp::Eq
                                                                }
                                                            }),
                                                            true_branch: Box::new(Expr {
                                                                kind: ExprKind::ConstantInt {
                                                                    value: 0.into()
                                                                }
                                                            }),
                                                            false_branch: Box::new(Expr {
                                                                kind: ExprKind::BinOp {
                                                                    left: Box::new(Expr {
                                                                        kind: ExprKind::ConstantInt {
                                                                            value: 8.into()
                                                                        }
                                                                    }),
                                                                    right: Box::new(Expr {
                                                                        kind: ExprKind::BinOp {
                                                                            left: Box::new(Expr {
                                                                                kind: ExprKind::Offset
                                                                            }),
                                                                            right: Box::new(Expr {
                                                                                kind: ExprKind::ConstantInt {
                                                                                    value: 8.into()
                                                                                }
                                                                            }),
                                                                            op: BinOp::Mod
                                                                        }
                                                                    }),
                                                                    op: BinOp::Sub
                                                                }
                                                            })
                                                        }
                                                    }
                                                },
                                                offset: None,
                                            }
                                        ),
                                    ],
                                },
                                offset: None,
                            }),
                            condition: Expr {
                                kind: ExprKind::BinOp {
                                    left: Box::new(Expr {
                                        kind: ExprKind::BinOp {
                                            left: Box::new(Expr {
                                                kind: ExprKind::FieldAccess {
                                                    field_holder: Box::new(Expr {
                                                        kind: ExprKind::Last
                                                    }),
                                                    field: "index_value_flags".into()
                                                }
                                            }),
                                            right: Box::new(Expr {
                                                kind: ExprKind::ConstantInt {
                                                    value: 2.into()
                                                }
                                            }),
                                            op: BinOp::And
                                        }
                                    }),
                                    right: Box::new(Expr {
                                        kind: ExprKind::ConstantInt {
                                            value: 2.into()
                                        }
                                    }),
                                    op: BinOp::Neq
                                }
                            }
                        },
                        offset: Some(Expr {
                            kind: ExprKind::BinOp {
                                left: Box::new(Expr {
                                    kind: ExprKind::FieldAccess {
                                        field_holder: Box::new(Expr {
                                            kind: ExprKind::VariableUse {
                                                var: "index_node_header".into()
                                            }
                                        }),
                                        field: "index_values_offset".into()
                                    }
                                }),
                                right: Box::new(Expr {
                                    kind: ExprKind::ConstantInt {
                                        value: 24.into()
                                    }
                                }),
                                op: BinOp::Add
                            }
                        }),
                    }
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_ntfs_header() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "boot_entry_point".into(),
                    Node {
                        kind: NodeKind::FixedLength {
                            length: Expr {
                                kind: ExprKind::ConstantInt { value: 3.into() },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "file_system_signature".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"NTFS    ".into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "bytes_per_sector".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "sectors_per_cluster_block".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 8,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "number_of_sectors".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: Some(Expr {
                            kind: ExprKind::ConstantInt { value: 40.into() },
                        }),
                    },
                ),
                (
                    "mft_cluster_block_number".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "mft_mirr_cluster_block_number".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "mft_entry_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 8,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "index_entry_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 8,
                            signed: false,
                        },
                        offset: Some(Expr {
                            kind: ExprKind::ConstantInt { value: 68.into() },
                        }),
                    },
                ),
                (
                    "volume_serial_number".into(),
                    Node {
                        kind: NodeKind::FixedLength {
                            length: Expr {
                                kind: ExprKind::ConstantInt { value: 8.into() },
                            },
                        },
                        offset: Some(Expr {
                            kind: ExprKind::ConstantInt { value: 72.into() },
                        }),
                    },
                ),
                (
                    "sector_signature".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"\x55\xaa".into(),
                                },
                            },
                        },
                        offset: Some(Expr {
                            kind: ExprKind::ConstantInt { value: 510.into() },
                        }),
                    },
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_bitlocker_header() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "boot_entry_point".into(),
                    Node {
                        kind: NodeKind::FixedLength {
                            length: Expr {
                                kind: ExprKind::ConstantInt { value: 3.into() },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "file_system_signature".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"-FVE-FS-".into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "bytes_per_sector".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "sectors_per_cluster_block".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 8,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "number_of_sectors".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: Some(Expr {
                            kind: ExprKind::ConstantInt { value: 40.into() },
                        }),
                    },
                ),
                (
                    "mft_cluster_block_number".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "mft_mirr_cluster_block_number".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "mft_entry_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 8,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "volume_serial_number".into(),
                    Node {
                        kind: NodeKind::FixedLength {
                            length: Expr {
                                kind: ExprKind::ConstantInt { value: 16.into() },
                            },
                        },
                        offset: Some(Expr {
                            kind: ExprKind::ConstantInt { value: 0xa0.into() },
                        }),
                    },
                ),
                (
                    "information_offsets".into(),
                    Node {
                        kind: NodeKind::RepeatCount {
                            node: Box::new(Node {
                                kind: NodeKind::Integer {
                                    bit_width: 64,
                                    signed: false,
                                },
                                offset: None
                            }),
                            count: Expr {
                                kind: ExprKind::ConstantInt {
                                    value: 3.into()
                                }
                            }
                        },
                        offset: Some(Expr {
                            kind: ExprKind::ConstantInt { value: 0xb0.into() },
                        }),
                    },
                ),
                (
                    "eow_information_offsets".into(),
                    Node {
                        kind: NodeKind::RepeatCount {
                            node: Box::new(Node {
                                kind: NodeKind::Integer {
                                    bit_width: 64,
                                    signed: false,
                                },
                                offset: None
                            }),
                            count: Expr {
                                kind: ExprKind::ConstantInt {
                                    value: 2.into()
                                }
                            }
                        },
                        offset: None,
                    },
                ),
                (
                    "sector_signature".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"\x55\xaa".into(),
                                },
                            },
                        },
                        offset: Some(Expr {
                            kind: ExprKind::ConstantInt { value: 510.into() },
                        }),
                    },
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_bitlocker_information() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "signature".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"-FVE-FS-".into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "version".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "current_state".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "next_state".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "encrypted_volume_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "convert_size".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "num_of_backup_sectors".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "information_offsets".into(),
                    Node {
                        kind: NodeKind::RepeatCount {
                            node: Box::new(Node {
                                kind: NodeKind::Integer {
                                    bit_width: 64,
                                    signed: false,
                                },
                                offset: None
                            }),
                            count: Expr {
                                kind: ExprKind::ConstantInt {
                                    value: 3.into()
                                }
                            }
                        },
                        offset: None,
                    },
                ),
                (
                    "boot_sector_backup".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "dataset".into(),
                    Node {
                        kind: NodeKind::Struct {
                            nodes: vec![
                                (
                                    "size".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false
                                        },
                                        offset: None
                                    }
                                ),
                                (
                                    "unknown1".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false
                                        },
                                        offset: None
                                    }
                                ),
                                (
                                    "header_size".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false
                                        },
                                        offset: None
                                    }
                                ),
                                (
                                    "copy_size".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false
                                        },
                                        offset: None
                                    }
                                ),
                                (
                                    "guid".into(),
                                    Node {
                                        kind: NodeKind::FixedLength {
                                            length: Expr {
                                                kind: ExprKind::ConstantInt {
                                                    value: 16.into(),
                                                }
                                            }
                                        },
                                        offset: None
                                    }
                                ),
                                (
                                    "next_counter".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false
                                        },
                                        offset: None
                                    }
                                ),
                                (
                                    "algorithm".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false
                                        },
                                        offset: None
                                    }
                                ),
                                (
                                    "trash".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false
                                        },
                                        offset: None
                                    }
                                ),
                                (
                                    "timestamp".into(),
                                    Node {
                                        kind: NodeKind::Integer {
                                            bit_width: 64,
                                            signed: false
                                        },
                                        offset: None
                                    }
                                ),
                            ]
                        },
                        offset: None
                    }
                ),
                (
                    "datums".into(),
                    Node {
                        kind: NodeKind::RepeatWhile {
                            node: Box::new(Node {
                                kind: NodeKind::Struct {
                                    nodes: vec![
                                        (
                                            "size".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false,
                                                },
                                                offset: None
                                            }
                                        ),
                                        (
                                            "entry_type".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false,
                                                },
                                                offset: None
                                            }
                                        ),
                                        (
                                            "value_type".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false,
                                                },
                                                offset: None
                                            }
                                        ),
                                        (
                                            "error_status".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 16,
                                                    signed: false,
                                                },
                                                offset: None
                                            }
                                        ),
                                        (
                                            "content".into(),
                                            Node {
                                                kind: NodeKind::Switch {
                                                    scrutinee: Expr {
                                                        kind: ExprKind::VariableUse {
                                                            var: "value_type".into(),
                                                        }
                                                    },
                                                    branches: vec![
                                                    ],
                                                    default: Box::new(Node {
                                                        kind: NodeKind::FixedLength {
                                                            length: Expr {
                                                                kind: ExprKind::BinOp {
                                                                    left: Box::new(Expr {
                                                                        kind: ExprKind::VariableUse {
                                                                            var: "size".into(),
                                                                        }
                                                                    }),
                                                                    right: Box::new(Expr {
                                                                        kind: ExprKind::ConstantInt {
                                                                            value: 8.into(),
                                                                        }
                                                                    }),
                                                                    op: BinOp::Sub
                                                                }
                                                            }
                                                        },
                                                        offset: None
                                                    })
                                                },
                                                offset: None
                                            }
                                        )
                                    ],
                                },
                                offset: None
                            }),
                            condition: Expr {
                                kind: ExprKind::BinOp {
                                    left: Box::new(Expr {
                                        kind: ExprKind::Offset,
                                    }),
                                    right: Box::new(Expr {
                                        kind: ExprKind::BinOp {
                                            left: Box::new(Expr {
                                                kind: ExprKind::FieldAccess {
                                                    field_holder: Box::new(Expr {
                                                        kind: ExprKind::VariableUse {
                                                            var: "dataset".into(),
                                                        }
                                                    }),
                                                    field: "size".into()
                                                },
                                            }),
                                            right: Box::new(Expr {
                                                kind: ExprKind::ConstantInt {
                                                    value: 0x40.into(),
                                                }
                                            }),
                                            op: BinOp::Add,
                                        }
                                    }),
                                    op: BinOp::Lt,
                                }
                            },
                        },
                        offset: None
                    }
                )
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_vhdx_region_table() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "signature".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"regi".into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "checksum".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "number_of_entries".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "reserved".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "entries".into(),
                    Node {
                        kind: NodeKind::RepeatCount {
                            node: Box::new(Node {
                                kind: NodeKind::Struct {
                                    nodes: vec![
                                        (
                                            "region_type_identifier".into(),
                                            Node {
                                                kind: NodeKind::FixedLength {
                                                    length: Expr {
                                                        kind: ExprKind::ConstantInt {
                                                            value: 16.into(),
                                                        },
                                                    },
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "region_data_offset".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 64,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "region_data_size".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "is_required_flag".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                    ],
                                },
                                offset: None,
                            }),
                            count: Expr {
                                kind: ExprKind::VariableUse {
                                    var: "number_of_entries".into(),
                                }
                            }
                        },
                        offset: None,
                    },
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_vhdx_metadata_table() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "signature".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"metadata".into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "unknown".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "number_of_entries".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "unknown2".into(),
                    Node {
                        kind: NodeKind::FixedLength {
                            length: Expr {
                                kind: ExprKind::ConstantInt {
                                    value: 20.into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "entries".into(),
                    Node {
                        kind: NodeKind::RepeatCount {
                            node: Box::new(Node {
                                kind: NodeKind::Struct {
                                    nodes: vec![
                                        (
                                            "metadata_item_identifier".into(),
                                            Node {
                                                kind: NodeKind::FixedLength {
                                                    length: Expr {
                                                        kind: ExprKind::ConstantInt {
                                                            value: 16.into(),
                                                        },
                                                    },
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "metadata_item_offset".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "metadata_item_size".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 32,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                        (
                                            "unknown".into(),
                                            Node {
                                                kind: NodeKind::Integer {
                                                    bit_width: 64,
                                                    signed: false,
                                                },
                                                offset: None,
                                            },
                                        ),
                                    ],
                                },
                                offset: None,
                            }),
                            count: Expr {
                                kind: ExprKind::VariableUse {
                                    var: "number_of_entries".into(),
                                }
                            }
                        },
                        offset: None,
                    },
                ),
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_vmdk_header() -> language::ast::Node {
    Node {
        kind: NodeKind::Struct {
            nodes: vec![
                (
                    "signature".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"KDMV".into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "version".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "flags".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "maximum_data_number_of_sectors".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "grain_number_of_sectors".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "descriptor_sector_number".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "descriptor_number_of_sectors".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "number_of_grains_table_entries".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 32,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "secondary_grain_directory_sector_number".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "grain_directory_sector_number".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "metadata_number_of_sectors".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 64,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "is_dirty".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 8,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "eol1".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"\n".into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "non_eol".into(),
                    Node {
                        kind: NodeKind::FixedLength {
                            length: Expr {
                                kind: ExprKind::ConstantInt {
                                    value: 1.into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "eol2".into(),
                    Node {
                        kind: NodeKind::FixedBytes {
                            expected: Expr {
                                kind: ExprKind::ConstantBytes {
                                    value: b"\r\n".into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
                (
                    "compression_method".into(),
                    Node {
                        kind: NodeKind::Integer {
                            bit_width: 16,
                            signed: false,
                        },
                        offset: None,
                    },
                ),
                (
                    "padding".into(),
                    Node {
                        kind: NodeKind::FixedLength {
                            length: Expr {
                                kind: ExprKind::ConstantInt {
                                    value: 433.into(),
                                },
                            },
                        },
                        offset: None,
                    },
                ),
            ],
        },
        offset: None,
    }
}
