//! Implements the parsing capabilities of hexbait.

use language::ast::BinOp;

pub mod eval;
pub mod language;

// TODO: implement a frontend for parsing `Node`s from text
// TODO: implement re-usable types
// TODO: implement "parse until" arrays
// TODO: implement display options (enum that name certain values)
// TODO: implement bitwise fields
// TODO: implement custom data streams
// TODO: implement classification of parsed values (offset, integer?, string?)
// TODO: improve display of the parsed values in the GUI
// TODO: figure out a way to cleverly incorporate colors
// TODO: implement different start offsets
// TODO: implement jumping to offsets in the hexview
// TODO: implement highlighting on the overview bars

// TODO: remove when this can be parsed from a text file
pub fn tmp_pe_file() -> language::ast::Node {
    use crate::parsing::language::ast::{Expr, ExprKind, Node, NodeKind};
    Node {
        name: "pe_file".into(),
        kind: NodeKind::Struct {
            nodes: vec![
                Node {
                    name: "mz_header".into(),
                    kind: NodeKind::Struct {
                        nodes: vec![
                            Node {
                                name: "mz_magic".into(),
                                kind: NodeKind::FixedBytes {
                                    expected: Expr {
                                        kind: ExprKind::ConstantBytes {
                                            value: b"MZ".into(),
                                        },
                                    },
                                },
                                offset: None,
                            },
                            Node {
                                name: "new_exe_header".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: Some(Expr {
                                    kind: ExprKind::ConstantInt { value: 60.into() },
                                }),
                            },
                        ],
                    },
                    offset: None,
                },
                Node {
                    name: "pe_header".into(),
                    kind: NodeKind::Struct {
                        nodes: vec![
                            Node {
                                name: "pe_magic".into(),
                                kind: NodeKind::FixedBytes {
                                    expected: Expr {
                                        kind: ExprKind::ConstantBytes {
                                            value: b"PE\0\0".into(),
                                        },
                                    },
                                },
                                offset: None,
                            },
                            Node {
                                name: "machine".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "num_of_sections".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "time_date_stamp".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "pointer_to_symbol_table".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "num_of_symbols".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "size_of_optional_header".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "characteristics".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
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
                Node {
                    name: "optional_header".into(),
                    kind: NodeKind::Struct {
                        nodes: vec![
                            Node {
                                name: "optional_magic".into(),
                                kind: NodeKind::FixedBytes {
                                    expected: Expr {
                                        kind: ExprKind::ConstantBytes {
                                            value: b"\x0b\x01".into(),
                                        },
                                    },
                                },
                                offset: None,
                            },
                            Node {
                                name: "major_linker_version".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 8,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "minor_linker_version".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 8,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "size_of_code".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "size_of_initialized_data".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "size_of_uninitialized_data".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "addr_of_entry_point".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "base_of_code".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "base_of_data".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                        ],
                    },
                    offset: None,
                },
                Node {
                    name: "sections".into(),
                    kind: NodeKind::RepeatCount {
                        node: Box::new(Node {
                            name: "section_header".into(),
                            kind: NodeKind::Struct {
                                nodes: vec![
                                    Node {
                                        name: "section_name".into(),
                                        kind: NodeKind::FixedLength {
                                            length: Expr {
                                                kind: ExprKind::ConstantInt { value: 8.into() },
                                            },
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "virtual_size".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "virtual_address".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "size_of_raw_data".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "pointer_to_raw_data".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "pointer_to_relocations".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "pointer_to_line_numbers".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "number_of_relocations".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "number_of_line_numbers".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "characteristics".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
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
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_mft_entry() -> language::ast::Node {
    use crate::parsing::language::ast::{Expr, ExprKind, Node, NodeKind};
    Node {
        name: "mft_entry".into(),
        kind: NodeKind::Struct {
            nodes: vec![
                Node {
                    name: "header".into(),
                    kind: NodeKind::Struct {
                        nodes: vec![
                            Node {
                                name: "magic".into(),
                                kind: NodeKind::FixedBytes {
                                    expected: Expr {
                                        kind: ExprKind::ConstantBytes {
                                            value: b"FILE".into(),
                                        },
                                    },
                                },
                                offset: None,
                            },
                            Node {
                                name: "fixup_value_offset".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "num_fixup_values".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "log_sequence_number".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 64,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "sequence_number".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "reference_count".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "first_attribute".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "entry_flags".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "used_entry_size".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "total_entry_size".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 32,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "file_reference".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 64,
                                    signed: false,
                                },
                                offset: None,
                            },
                            Node {
                                name: "first_attribute_identifier".into(),
                                kind: NodeKind::Integer {
                                    bit_width: 16,
                                    signed: false,
                                },
                                offset: None,
                            },
                        ],
                    },
                    offset: None,
                },
                Node {
                    name: "attributes".into(),
                    kind: NodeKind::RepeatCount {
                        node: Box::new(Node {
                            name: "attribute".into(),
                            kind: NodeKind::Struct {
                                nodes: vec![
                                    Node {
                                        name: "type".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "size".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 32,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "nonresident".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 8,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "name_size".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 8,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "name_offset".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "data_flags".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "identifier".into(),
                                        kind: NodeKind::Integer {
                                            bit_width: 16,
                                            signed: false,
                                        },
                                        offset: None,
                                    },
                                    Node {
                                        name: "content".into(),
                                        kind: NodeKind::FixedLength {
                                            length: Expr {
                                                kind: ExprKind::BinOp {
                                                    left: Box::new(Expr {
                                                        kind: ExprKind::VariableUse {
                                                            var: "size".into(),
                                                        },
                                                    }),
                                                    right: Box::new(Expr {
                                                        kind: ExprKind::ConstantInt {
                                                            value: 16.into(),
                                                        },
                                                    }),
                                                    op: BinOp::Sub,
                                                },
                                            },
                                        },
                                        offset: None,
                                    },
                                ],
                            },
                            offset: None,
                        }),
                        count: Expr {
                            kind: ExprKind::ConstantInt { value: 4.into() },
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
            ],
        },
        offset: None,
    }
}

// TODO: remove when this can be parsed from a text file
pub fn tmp_ntfs_header() -> language::ast::Node {
    use crate::parsing::language::ast::{Expr, ExprKind, Node, NodeKind};
    Node {
        name: "ntfs_header".into(),
        kind: NodeKind::Struct {
            nodes: vec![
                Node {
                    name: "boot_entry_point".into(),
                    kind: NodeKind::FixedLength {
                        length: Expr {
                            kind: ExprKind::ConstantInt { value: 3.into() },
                        },
                    },
                    offset: None,
                },
                Node {
                    name: "file_system_signature".into(),
                    kind: NodeKind::FixedBytes {
                        expected: Expr {
                            kind: ExprKind::ConstantBytes {
                                value: b"NTFS    ".into(),
                            },
                        },
                    },
                    offset: None,
                },
                Node {
                    name: "bytes_per_sector".into(),
                    kind: NodeKind::Integer {
                        bit_width: 16,
                        signed: false,
                    },
                    offset: None,
                },
                Node {
                    name: "sectors_per_cluster_block".into(),
                    kind: NodeKind::Integer {
                        bit_width: 8,
                        signed: false,
                    },
                    offset: None,
                },
                Node {
                    name: "number_of_sectors".into(),
                    kind: NodeKind::Integer {
                        bit_width: 64,
                        signed: false,
                    },
                    offset: Some(Expr {
                        kind: ExprKind::ConstantInt { value: 40.into() },
                    }),
                },
                Node {
                    name: "mft_cluster_block_number".into(),
                    kind: NodeKind::Integer {
                        bit_width: 64,
                        signed: false,
                    },
                    offset: None,
                },
                Node {
                    name: "mft_mirr_cluster_block_number".into(),
                    kind: NodeKind::Integer {
                        bit_width: 64,
                        signed: false,
                    },
                    offset: None,
                },
                Node {
                    name: "mft_entry_size".into(),
                    kind: NodeKind::Integer {
                        bit_width: 8,
                        signed: false,
                    },
                    offset: None,
                },
                Node {
                    name: "index_entry_size".into(),
                    kind: NodeKind::Integer {
                        bit_width: 8,
                        signed: false,
                    },
                    offset: Some(Expr {
                        kind: ExprKind::ConstantInt { value: 68.into() },
                    }),
                },
                Node {
                    name: "volume_serial_number".into(),
                    kind: NodeKind::FixedLength {
                        length: Expr {
                            kind: ExprKind::ConstantInt { value: 8.into() },
                        },
                    },
                    offset: Some(Expr {
                        kind: ExprKind::ConstantInt { value: 72.into() },
                    }),
                },
                Node {
                    name: "sector_signature".into(),
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
            ],
        },
        offset: None,
    }
}
