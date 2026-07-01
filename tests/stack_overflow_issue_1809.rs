//! Regression test for https://github.com/async-graphql/async-graphql/issues/1809
//!
//! A deeply nested or wide GraphQL object hierarchy should not cause a stack
//! overflow when the `boxed-trait` feature is **not** enabled. The fix is to
//! box the futures returned by `resolve_field_async` and
//! `resolve_simple_field_value` so that each level of resolution is
//! heap-allocated rather than growing the stack frame.

use gqlrs::*;

// ── nested objects (depth) ────────────────────────────────────────────

struct Leaf {
    value: i32,
}

#[Object]
impl Leaf {
    async fn value(&self) -> i32 {
        self.value
    }
}

struct Branch {
    leaf: Leaf,
}

#[Object]
impl Branch {
    async fn leaf(&self) -> &Leaf {
        &self.leaf
    }
}

struct Trunk {
    branch: Branch,
}

#[Object]
impl Trunk {
    async fn branch(&self) -> &Branch {
        &self.branch
    }
}

struct Tree {
    trunk: Trunk,
}

#[Object]
impl Tree {
    async fn trunk(&self) -> &Trunk {
        &self.trunk
    }
}

struct Forest {
    trees: Vec<Tree>,
}

#[Object]
impl Forest {
    async fn trees(&self) -> &[Tree] {
        &self.trees
    }
}

struct World {
    forest: Forest,
}

#[Object]
impl World {
    async fn forest(&self) -> &Forest {
        &self.forest
    }
}

struct Query;

#[Object]
impl Query {
    async fn world(&self) -> World {
        World {
            forest: Forest {
                trees: (0..20)
                    .map(|i| Tree {
                        trunk: Trunk {
                            branch: Branch {
                                leaf: Leaf { value: i },
                            },
                        },
                    })
                    .collect(),
            },
        }
    }
}

/// A query that is 4 objects deep and fetches 20 top-level items.
/// Each item must produce a nested resolution chain:
///   Query -> Forest -> Tree[i] -> Trunk -> Branch -> Leaf -> value
#[tokio::test]
async fn test_deeply_nested_resolution_does_not_overflow() {
    let schema = Schema::new(Query, EmptyMutation, EmptySubscription);

    let result = schema
        .execute("{ world { forest { trees { trunk { branch { leaf { value } } } } } } }")
        .await
        .into_result()
        .expect("query should not error");

    // The entire data payload should match the expected structure.
    let expected_trees: Vec<Value> = (0..20)
        .map(|i| {
            value!({
                "trunk": {
                    "branch": {
                        "leaf": {
                            "value": i
                        }
                    }
                }
            })
        })
        .collect();

    assert_eq!(
        result.data,
        value!({
            "world": {
                "forest": {
                    "trees": expected_trees
                }
            }
        })
    );
}
