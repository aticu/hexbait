use std::{collections::HashMap, env, fs, path::PathBuf};
use ungrammar::{Grammar, Node, Rule, Token};

fn main() {
    println!("cargo:rerun-if-changed=grammar.ungram");
    let g = fs::read_to_string("grammar.ungram").unwrap();
    let grammar: Grammar = g.parse().unwrap();

    println!("cargo:rerun-if-changed=tokens.mappings");
    let m = fs::read_to_string("tokens.mappings").unwrap();
    let mappings: HashMap<&str, &str> = m
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.starts_with("//") && !line.is_empty())
        .map(|line| {
            let mut iter = line.split(" => ");
            let token = iter
                .next()
                .expect("mappings should be of form `ungrammar_token => RustTokenName`");
            let rust_name = iter
                .next()
                .expect("mappings should be of form `ungrammar_token => RustTokenName`");
            (token, rust_name)
        })
        .collect();

    let mut out = String::new();

    // Nodes
    for node in grammar.iter() {
        let name = &grammar[node].name;
        let rule = &grammar[node].rule;

        let mut impl_body = String::new();

        enum ImplKind {
            Struct,
            Enum { variants: Vec<(String, Node)> },
        }

        let impl_kind = match rule {
            Rule::Labeled { label, rule } => {
                if !matches!(&**rule, Rule::Token(_)) {
                    panic!("top level lables only supported for tokens");
                }

                impl_body.push_str(&format!(
                    r#"
    /// Returns the child token.
    pub fn {label}(&self) -> Option<SyntaxToken> {{
        self.0.children_with_tokens().filter_map(|it| it.into_token()).next()
    }}"#,
                ));

                ImplKind::Struct
            }
            Rule::Node(_) => panic!("top level nodes not supported"),
            Rule::Token(_) => {
                impl_body.push_str(
                    r#"
    /// Returns the child token.
    pub fn child(&self) -> Option<SyntaxToken> {
        self.0.children_with_tokens().filter_map(|it| it.into_token()).next()
    }"#,
                );

                ImplKind::Struct
            }
            Rule::Seq(rules) => {
                let mut node_counts = HashMap::new();
                let mut token_counts = HashMap::new();
                for rule in rules {
                    let Some(named_result) = find_only_named_rule(&grammar, rule) else {
                        if let Rule::Token(token) = rule {
                            let mapped = mappings.get(&*grammar[*token].name);
                            *token_counts
                                .entry(mapped.unwrap_or(&"unmapped"))
                                .or_insert(0u32) += 1;
                        }
                        continue;
                    };

                    let name = method_name(named_result.name);
                    let (mut body, return_ty, count) = match named_result.result {
                        NodeOrToken::Node(node) => {
                            let count = node_counts.entry(node).or_insert(0u32);
                            let current_count = *count;
                            *count += 1;
                            (
                                String::from("children(self.syntax())"),
                                &*grammar[node].name,
                                current_count,
                            )
                        }
                        NodeOrToken::Token(token) => {
                            let mapped = &mappings[&*grammar[token].name];
                            let count = token_counts.entry(mapped).or_insert(0u32);
                            let current_count = *count;
                            *count += 1;
                            (
                                format!("tokens(self.syntax(), {})", mapped),
                                "SyntaxToken",
                                current_count,
                            )
                        }
                    };

                    if named_result.single_occurrence {
                        body.push_str(&format!(".nth({count})"));
                    }
                    let return_ty = if named_result.single_occurrence {
                        format!("Option<{return_ty}>")
                    } else {
                        format!("impl Iterator<Item = {return_ty}>")
                    };

                    impl_body.push_str(&format!(
                        r#"
    /// Returns the node for [`{name}`].
    pub fn {name}(&self) -> {return_ty} {{
        {body}
    }}"#
                    ));
                }

                ImplKind::Struct
            }
            Rule::Alt(rules) => {
                // we accept too types of alt rules: one where all variants contain a (possibly
                // named) node and one where all variants are unnamed tokens

                let all_tokens = rules.iter().all(|rule| matches!(rule, Rule::Token(_)));

                if all_tokens {
                    // if we have all tokens, we can just wrap that token into a single struct
                    // we don't need to ensure that the types match up, because the parser should
                    // already ensure this
                    impl_body.push_str(
                        r#"
    /// Returns the child token.
    pub fn child(&self) -> Option<SyntaxToken> {
        self.0.children_with_tokens().filter_map(|it| it.into_token()).next()
    }

    /// Returns the child token.
    pub fn child_kind(&self) -> Option<TokenKind> {
        match self.child()?.kind() {
            SyntaxKind::Token { kind } => Some(kind),
            _ => unreachable!("tokens always have `SyntaxKind::Token` as `SyntaxKind`")
        }
    }"#,
                    );

                    ImplKind::Struct
                } else {
                    let mut variants = Vec::new();
                    for rule in rules {
                        let named_result = find_only_named_rule(&grammar, rule)
                            .expect("expected nodes as alt variations");
                        let NodeOrToken::Node(node) = named_result.result else {
                            panic!("expected nodes as alt variations");
                        };
                        variants.push((named_result.name.to_string(), node));
                    }

                    ImplKind::Enum { variants }
                }
            }
            Rule::Opt(_) => panic!("top level opt not supported"),
            Rule::Rep(rule) => {
                let named_result = find_only_named_rule(&grammar, rule)
                    .expect("expected nodes as repetition units");
                let name = method_name(named_result.name);
                let NodeOrToken::Node(node) = named_result.result else {
                    panic!("expected nodes as repetition units");
                };
                let node_name = &grammar[node].name;

                impl_body.push_str(&format!(
                    r#"
    /// Returns nodes of the repetition [`{name}`].
    pub fn {name}(&self) -> impl Iterator<Item = {node_name}> {{
        children(self.syntax())
    }}"#
                ));

                ImplKind::Struct
            }
        };

        match impl_kind {
            ImplKind::Struct => {
                out.push_str(&format!(
                    r#"
/// Represents the [`{name}`] AST node.
#[derive(Debug, Clone)]
pub struct {name}(SyntaxNode);

impl AstNode for {name} {{
    fn cast(n: SyntaxNode) -> Option<Self> {{
        if n.kind() == (SyntaxKind::Node {{ kind: NodeKind::{name} }}) {{
            Some({name}(n))
        }} else {{
            None
        }}
    }}

    fn syntax(&self) -> &SyntaxNode {{
        &self.0
    }}
}}

impl {name} {{{impl_body}
}}
"#
                ));
            }
            ImplKind::Enum { variants } => {
                let mut variant_descriptions = String::new();
                let mut casts = String::new();
                let mut syntax_extraction_arms = String::new();
                let enum_name = name;

                for (name, node) in variants {
                    let node_name = &grammar[node].name;
                    variant_descriptions.push_str(&format!(
                        r#"
    /// The [`{name}`] variant of the [`{enum_name}`] AST node.
    {name}({node_name}),"#
                    ));

                    casts.push_str(&format!(
                        r#"
        if let Some(inner) = {node_name}::cast(n.clone()) {{
            return Some(Self::{name}(inner));
        }}"#
                    ));

                    syntax_extraction_arms.push_str(&format!(
                        r#"
            Self::{name}(inner) => inner.syntax(),"#
                    ));
                }

                out.push_str(&format!(
                    r#"
/// Represents the [`{name}`] AST node.
#[derive(Debug, Clone)]
pub enum {name} {{{variant_descriptions}
}}

impl AstNode for {name} {{
    fn cast(n: SyntaxNode) -> Option<Self> {{{casts}
        None
    }}

    fn syntax(&self) -> &SyntaxNode {{
        match self {{{syntax_extraction_arms}
        }}
    }}
}}

impl {name} {{{impl_body}
}}
"#
                ));
            }
        }
    }

    // write
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out_dir.join("ast.gen.rs"), out).unwrap();
}

/// Either a node or a token.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
enum NodeOrToken {
    /// A node.
    Node(Node),
    /// A token.
    Token(Token),
}

/// A finding of a named rule.
struct NamedResult<'grammar> {
    /// The name of the rule.
    name: &'grammar str,
    /// The named rule that was found.
    result: NodeOrToken,
    /// Whether or not we expect a single occurrence of the result.
    single_occurrence: bool,
}

/// Finds the only named rule.
fn find_only_named_rule<'grammar>(
    grammar: &'grammar Grammar,
    rule: &'grammar Rule,
) -> Option<NamedResult<'grammar>> {
    match rule {
        Rule::Labeled { label, rule } => match &**rule {
            Rule::Node(node) => Some(NamedResult {
                name: label,
                result: NodeOrToken::Node(*node),
                single_occurrence: true,
            }),
            Rule::Token(token) => Some(NamedResult {
                name: label,
                result: NodeOrToken::Token(*token),
                single_occurrence: true,
            }),
            Rule::Labeled { .. } | Rule::Seq(_) | Rule::Alt(_) | Rule::Opt(_) | Rule::Rep(_) => {
                dbg!(&rule);
                panic!("labels only supported on tokens and nodes")
            }
        },
        Rule::Node(node) => Some(NamedResult {
            name: &grammar[*node].name,
            result: NodeOrToken::Node(*node),
            single_occurrence: true,
        }),
        Rule::Token(_) => None,
        Rule::Seq(rules) => {
            let mut result = None;

            for rule in rules {
                let new_result = find_only_named_rule(grammar, rule);
                if new_result.is_some() {
                    if result.is_some() {
                        panic!("found more than one named rule where only one was expected")
                    } else {
                        result = new_result;
                    }
                }
            }

            result
        }
        Rule::Alt(_) => None,
        Rule::Opt(rule) => find_only_named_rule(grammar, rule),
        Rule::Rep(rule) => find_only_named_rule(grammar, rule).map(|result| NamedResult {
            single_occurrence: false,
            ..result
        }),
    }
}

/// Returns the given name as a method.
fn method_name(name: &str) -> String {
    format!("{}", heck::AsSnakeCase(name))
}
