mod utils;

#[cfg(test)]
mod tests {

    use hexroll3_scroll::generators::*;
    use hexroll3_scroll::instance::*;
    use hexroll3_scroll::renderer::*;

    use crate::utils::create_tempfile;

    trait TestEntity {
        fn first_in(&self, attr: &str) -> Option<&str>;
        // fn has(&self, attr: &str) -> bool;
    }

    impl TestEntity for serde_json::Value {
        fn first_in(&self, attr: &str) -> Option<&str> {
            self[attr].as_array().unwrap().first().unwrap().as_str()
        }

        // fn has(&self, attr: &str) -> bool {
        //     self.as_object().unwrap().contains_key(attr)
        // }
    }

    // ------------------------------------------------------------------------
    #[test]
    fn test_class_hierarchy() {
        let instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {}
class2(class1) {}
class3(class2) {}
",
        );
        let hierarchy = &instance
            .resolve_class(&mut blueprint, "class3")
            .unwrap()
            .hierarchy;
        assert_eq!(hierarchy.len(), 3);
        assert!(hierarchy.first().unwrap() == "class3");
        assert!(hierarchy.get(1).unwrap() == "class2");
        assert!(hierarchy.get(2).unwrap() == "class1");
    }

    // ------------------------------------------------------------------------
    #[test]
    fn test_roll_value_from_list() {
        let mut instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {
    value @ [
        * a
        * b
        * c
    ]
}",
        );
        let tmp = create_tempfile();
        instance.repo.create(tmp.path().to_str().unwrap()).unwrap();
        let generated_ids = instance
            .repo
            .mutate(|tx| {
                roll(
                    &SandboxBuilder::from_instance(&instance),
                    &mut blueprint,
                    tx,
                    "class1",
                    "root",
                    None,
                )
            })
            .unwrap();
        let generated_root = instance.repo.load(&generated_ids).unwrap();
        assert!(
            ["a", "b", "c"]
                .contains(&generated_root["value"].as_str().unwrap())
        );
    }

    // ------------------------------------------------------------------------
    #[test]
    fn test_strings_numbers_and_dice() {
        let mut instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {
    test1 = This is a string
    test2 = \"This is another string\"
    test3 = <% this is a template %>
    test4 ~ <% this is another template %>
    test5 = 10
    test6 = 10.0
    test7 @ 2d20+1
}",
        );
        let tmp = create_tempfile();
        instance.repo.create(tmp.path().to_str().unwrap()).unwrap();
        let _generated_id = instance
            .repo
            .mutate(|tx| {
                roll(
                    &SandboxBuilder::from_instance(&instance),
                    &mut blueprint,
                    tx,
                    "class1",
                    "root",
                    None,
                )
            })
            .unwrap();
    }

    // ------------------------------------------------------------------------
    #[test]
    fn test_rolling_a_list() {
        let mut instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {}

class2 {
    [3..10 list] @ class1
}",
        );
        let tmp = create_tempfile();
        instance.repo.create(tmp.path().to_str().unwrap()).unwrap();
        let generated_ids = instance
            .repo
            .mutate(|tx| {
                roll(
                    &SandboxBuilder::from_instance(&instance),
                    &mut blueprint,
                    tx,
                    "class2",
                    "root",
                    None,
                )
            })
            .unwrap();
        let generated_root = instance.repo.load(&generated_ids).unwrap();
        let list_length = generated_root["list"].as_array().unwrap().len();
        assert!((3..=10).contains(&list_length));
    }

    // ------------------------------------------------------------------------
    #[test]
    fn test_rolling_an_entity_using_indirection() {
        let mut instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {}

class2 {
    indirection = class1
    entity @ &indirection
}",
        );
        let tmp = create_tempfile();
        instance.repo.create(tmp.path().to_str().unwrap()).unwrap();
        let generated_ids = instance
            .repo
            .mutate(|tx| {
                roll(
                    &SandboxBuilder::from_instance(&instance),
                    &mut blueprint,
                    tx,
                    "class2",
                    "root",
                    None,
                )
            })
            .unwrap();
        let generated_root = instance.repo.load(&generated_ids).unwrap();
        let generated_entity = instance
            .repo
            .load(generated_root.first_in("entity").unwrap())
            .unwrap();
        assert_eq!(generated_entity["class"], "class1");
    }

    // ------------------------------------------------------------------------
    #[test]
    fn test_context_attribute() {
        let mut instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {
    value1! = :class2.foo
    value2! = *class2.foo
}

class2 {
    foo = bar
    child! @ class1
}",
        );
        let tmp = create_tempfile();
        instance.repo.create(tmp.path().to_str().unwrap()).unwrap();
        let generated_ids = instance
            .repo
            .mutate(|tx| {
                roll(
                    &SandboxBuilder::from_instance(&instance),
                    &mut blueprint,
                    tx,
                    "class2",
                    "root",
                    None,
                )
            })
            .unwrap();
        let generated_root = instance.repo.load(&generated_ids).unwrap();
        let rendered_result = instance
            .repo
            .inspect(|tx| {
                render_entity(
                    &instance,
                    &mut blueprint,
                    tx,
                    &generated_root,
                    false,
                )
            })
            .unwrap();
        assert_eq!(rendered_result["child"]["value1"], "bar");
        assert_eq!(rendered_result["child"]["value2"], "bar");
    }

    // ------------------------------------------------------------------------
    #[test]
    fn test_pointer_attribute() {
        let mut instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {
    injected! = null
}

class2 {
    foo = bar
    child! @ class1 {
        injected = *foo
    }
    output! = <% {{child.injected}} %>
}",
        );
        let tmp = create_tempfile();
        instance.repo.create(tmp.path().to_str().unwrap()).unwrap();
        let generated_ids = instance
            .repo
            .mutate(|tx| {
                roll(
                    &SandboxBuilder::from_instance(&instance),
                    &mut blueprint,
                    tx,
                    "class2",
                    "root",
                    None,
                )
            })
            .unwrap();
        let generated_root = instance.repo.load(&generated_ids).unwrap();
        let rendered_result = instance
            .repo
            .inspect(|tx| {
                render_entity(
                    &instance,
                    &mut blueprint,
                    tx,
                    &generated_root,
                    false,
                )
            })
            .unwrap();
        assert_eq!(rendered_result["output"], "bar");
    }

    // ------------------------------------------------------------------------
    #[test]
    fn test_use_from_collection() {
        let mut instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {
    foo = bar
}

class2 {
    xyz = abc
    use % class1 {
        xyz = &xyz
    }
}

class3 {
    << class1
    a @ class1
    b @ class2
}",
        );
        let tmp = create_tempfile();
        instance.repo.create(tmp.path().to_str().unwrap()).unwrap();
        instance
            .repo
            .mutate(|tx| {
                roll(
                    &SandboxBuilder::from_instance(&instance),
                    &mut blueprint,
                    tx,
                    "class3",
                    "root",
                    None,
                )
            })
            .and_then(|generated_ids| {
                instance.repo.inspect(|tx| {
                    let a = tx.fetch(&generated_ids).unwrap().clone();
                    let b = tx.fetch(a.first_in("b").unwrap()).unwrap().clone();
                    let c =
                        tx.fetch(b.first_in("use").unwrap()).unwrap().clone();
                    assert_eq!(b["use"], a["a"]);
                    assert_eq!(c["xyz"], "abc");
                    Ok(())
                })
            })
            .unwrap();
    }
    // ------------------------------------------------------------------------
    #[test]
    fn test_unroll_removes_entities() {
        let mut instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {
    foo = bar
}

class2 {
    a @ class1
}

class3 {
    b @ class2
}",
        );

        let tmp = create_tempfile();
        let mut v: Vec<String> = Vec::new();
        instance.repo.create(tmp.path().to_str().unwrap()).unwrap();
        instance
            .repo
            .mutate(|tx| {
                roll(
                    &SandboxBuilder::from_instance(&instance),
                    &mut blueprint,
                    tx,
                    "class3",
                    "root",
                    None,
                )
            })
            .and_then(|generated_id| {
                instance.repo.inspect(|tx| {
                    let e3 = tx.fetch(&generated_id).unwrap().clone();
                    let e2 =
                        tx.fetch(e3.first_in("b").unwrap()).unwrap().clone();
                    v.push(e3.first_in("b").unwrap().to_string());
                    v.push(e2.first_in("a").unwrap().to_string());
                    v.push(generated_id.to_string());
                    Ok(generated_id.clone())
                })
            })
            .and_then(|generated_id| {
                instance.repo.mutate(|tx| {
                    unroll(
                        &SandboxBuilder::from_instance(&instance),
                        &mut blueprint,
                        tx,
                        &generated_id,
                        None,
                    )
                })
            })
            .and_then(|_| {
                instance.repo.inspect(|tx| {
                    for i in v.iter() {
                        assert!(tx.load(i).is_err());
                    }
                    Ok(())
                })
            })
            .unwrap();
    }
    // ------------------------------------------------------------------------
    #[test]
    fn test_unroll_clears_a_user() {
        let mut instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {
    foo = bar
}

class2 {
    xyz = abc
    use % class1 {
        xyz = &xyz
    }
}

class3 {
    << class1
    a @ class1
    b @ class2
}",
        );
        let tmp = create_tempfile();
        instance.repo.create(tmp.path().to_str().unwrap()).unwrap();
        instance
            .repo
            .mutate(|tx| {
                roll(
                    &SandboxBuilder::from_instance(&instance),
                    &mut blueprint,
                    tx,
                    "class3",
                    "root",
                    None,
                )
            })
            .and_then(|generated_id| {
                instance.repo.inspect(|tx| {
                    let c3 = tx.fetch(&generated_id).unwrap().clone();
                    let a = c3.first_in("a").unwrap().to_string();
                    Ok(a)
                })
            })
            .and_then(|a| {
                instance.repo.mutate(|tx| {
                    unroll(
                        &SandboxBuilder::from_instance(&instance),
                        &mut blueprint,
                        tx,
                        &a,
                        None,
                    )
                })
            })
            .and_then(|parent_id| {
                instance.repo.inspect(|tx| {
                    let c3 = tx.fetch(&parent_id).unwrap().clone();
                    assert!(c3["a"].as_array().unwrap().is_empty());
                    let b =
                        tx.fetch(c3.first_in("b").unwrap()).unwrap().clone();
                    assert!(b["use"].as_array().unwrap().is_empty());
                    Ok(())
                })
            })
            .unwrap();
    }
    // ------------------------------------------------------------------------
    #[test]
    fn test_reroll() {
        let mut instance = SandboxInstance::new();
        let mut blueprint = instance.blueprint.lock().unwrap();
        instance.parse_buffer(
            &mut blueprint,
            "
class1 {
    foo = bar
}

class2 {
    xyz = abc
    use ? class1 {
        abc = 1
        xyz = &xyz
        foo = \"bar\"
        bar @ [
            * f
            * o
            * o
        ]

    }
}

class3 {
    << class1
    a @ class1
    b @ class2
}",
        );
        let tmp = create_tempfile();
        instance.repo.create(tmp.path().to_str().unwrap()).unwrap();
        instance
            .repo
            .mutate(|tx| {
                roll(
                    &SandboxBuilder::from_instance(&instance),
                    &mut blueprint,
                    tx,
                    "class3",
                    "root",
                    None,
                )
            })
            .and_then(|generated_id| {
                instance.repo.inspect(|tx| {
                    let c3 = tx.fetch(&generated_id).unwrap().clone();
                    let a = c3.first_in("a").unwrap().to_string();
                    Ok(a)
                })
            })
            .and_then(|a| {
                instance.repo.mutate(|tx| {
                    reroll(
                        &SandboxBuilder::from_instance(&instance),
                        &mut blueprint,
                        tx,
                        &a,
                        None,
                    )
                })
            })
            .and_then(|rerolled_id| {
                instance.repo.inspect(|tx| {
                    let c1 = tx.fetch(&rerolled_id).unwrap().clone();
                    Ok(c1["parent_uid"].as_str().unwrap().to_owned())
                })
            })
            .and_then(|parent_id| {
                instance.repo.inspect(|tx| {
                    let c3 = tx.fetch(&parent_id).unwrap().clone();
                    assert!(c3["a"].as_array().unwrap().len() == 1);
                    let b =
                        tx.fetch(c3.first_in("b").unwrap()).unwrap().clone();
                    assert!(
                        b["use"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .next()
                            .unwrap()
                            .as_str()
                            .unwrap()
                            == c3["a"][0].as_str().unwrap()
                    );
                    Ok(())
                })
            })
            .unwrap();
    }
    // ------------------------------------------------------------------------
    #[test]
    fn test_create_instance() {
        let tmp = create_tempfile();
        let mut instance = SandboxInstance::new();
        {
            let mut blueprint = instance.blueprint.lock().unwrap();
            instance.parse_buffer(
                &mut blueprint,
                "
main {
}",
            );
        }
        {
            instance.create(tmp.path().to_str().unwrap()).unwrap();
        }
        {
            let instance = SandboxInstance::new();
            let mut blueprint = instance.blueprint.lock().unwrap();
            instance.parse_buffer(
                &mut blueprint,
                "
        main {
        }",
            );
        }
        {
            instance.open(tmp.path().to_str().unwrap()).unwrap();
        }
    }
}
