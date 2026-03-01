mod utils;

#[cfg(test)]
mod renderer {
    use hexroll3_scroll::renderer::render_entity;
    use minijinja::Environment;

    use hexroll3_scroll::instance::*;
    use hexroll3_scroll::renderer_env::*;

    use crate::utils::create_tempfile;

    fn render(template: &str) -> String {
        let mut env = Environment::new();
        let instance = SandboxInstance::new();
        prepare_renderer(&mut env, &instance);
        env.add_template("test", template).unwrap();
        let tmpl = env.get_template("test").unwrap();
        tmpl.render(serde_json::json!({})).unwrap()
    }

    #[test]
    fn test_func_empty() {
        let result = render("{{note_button('test')}}");
        assert_eq!(result, "");
    }

    #[test]
    fn test_func_stable_dice() {
        let result_a = render("{{stable_dice('1d100','uuid',1)}}");
        let result_b = render("{{stable_dice('1d100','uuid',1)}}");
        assert_eq!(result_a, result_b);
    }

    #[test]
    fn test_func_max() {
        assert_eq!(render("{{max(5,1)}}"), "5");
        assert_eq!(render("{{max(-10,1)}}"), "1");
    }

    #[test]
    fn test_func_plural() {
        assert_eq!(render("{{plural(3,'wraith')}}"), "wraithes");
        assert_eq!(render("{{plural(4,'orc')}}"), "orcs");
        assert_eq!(render("{{plural(5,'wolf')}}"), "wolves");
        assert_eq!(render("{{plural(6,'fly')}}"), "flies");
        assert_eq!(render("{{plural(1,'orc')}}"), "orc");
    }

    #[test]
    fn test_func_length() {
        assert_eq!(render("{{length('zero')}}"), "0");
        assert_eq!(render("{{length([1,2,3])}}"), "3");
    }

    #[test]
    fn test_func_trim() {
        assert_eq!(render("{{trim(' core  ')}}"), "core");
        assert_eq!(render("{{trim('core ')}}"), "core");
        assert_eq!(render("{{trim('  core')}}"), "core");
        assert_eq!(render("{{trim('     ')}}"), "");
    }

    #[test]
    fn test_func_articlize() {
        assert_eq!(render("{{articlize('sword')}}"), "a sword");
        assert_eq!(render("{{articlize('axe')}}"), "an axe");
        assert_eq!(render("{{articlize('swords')}}"), "swords");
        assert_eq!(render("{{articlize('axes')}}"), "axes");
    }

    #[test]
    fn test_func_unique() {
        assert_eq!(
            render("{{unique([{'a':'a'},{'a':'a'}],'a')}}"),
            "[{\"a\": \"a\"}]"
        );
    }

    #[test]
    fn test_func_capitalize() {
        assert_eq!(render("{{capitalize('fighter')}}"), "Fighter");
    }

    #[test]
    fn test_func_currency() {
        assert_eq!(render("{{currency(1000)}}"), "1,000 gp");
        assert_eq!(render("{{currency(3.5)}}"), "3 gp");
        assert_eq!(render("{{currency(0.5)}}"), "5 sp");
        assert_eq!(render("{{currency(0.05)}}"), "5 cp");
    }

    #[test]
    fn test_func_first() {
        assert_eq!(render("{{first([1,2,3])}}"), "1");
    }

    #[test]
    fn test_recursive_render() {
        let mut instance = SandboxInstance::new();
        instance.parse_buffer(
            "
Hex {
    Description! = <%foo%>
    realm = *Realm.Name
}

Region {
    [3..3 hexes] @ Hex
    foo = bar
    xyz = `foo`
}

Realm {
    [3..3 regions] @ Region
    name! = \"bar\"
}

main {
    realm @ Realm
    hexes << Hex
    output! ~ <%{{realm.name}}%>
}
",
        );
        let tmp = create_tempfile();
        instance.create(tmp.path().to_str().unwrap()).unwrap();
        instance
            .repo
            .inspect(|tx| {
                let main = tx.fetch("root").unwrap();
                Ok(main.clone())
            })
            .and_then(|realm_uid| instance.repo.load(&realm_uid.as_str().unwrap()))
            .and_then(|main| {
                let rendered_result = instance
                    .repo
                    .inspect(|tx| render_entity(&instance, tx, &main, false))
                    .unwrap();
                assert_eq!(rendered_result["output"], "bar");
                Ok(())
            })
            .unwrap();
    }
}
