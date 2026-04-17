pub mod model;
pub mod parser;

pub use model::*;
pub use parser::Parser;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_includes_simple() {
        let mut parser = Parser::new().unwrap();
        let project = parser
            .parse_file(Path::new("tests/fixtures/includes_simple.c"))
            .unwrap();

        assert_eq!(project.files.len(), 1);
        let file = project.files.values().next().unwrap();

        let include_data: Vec<(&str, bool, _)> = file.iter_all_includes().collect();

        // Should parse all 7 includes
        assert_eq!(
            include_data.len(),
            7,
            "Expected 7 includes, got {}",
            include_data.len()
        );

        assert_eq!(include_data[0].0, "config.h");
        assert!(!include_data[0].1, "config.h should not be system");

        assert_eq!(include_data[1].0, "math.h");
        assert!(include_data[1].1, "math.h should be system");

        assert_eq!(include_data[2].0, "stdio.h");
        assert!(include_data[2].1, "stdio.h should be system");

        assert_eq!(include_data[3].0, "glib.h");
        assert!(include_data[3].1, "glib.h should be system");

        assert_eq!(include_data[4].0, "gtk/gtk.h");
        assert!(include_data[4].1, "gtk/gtk.h should be system");

        assert_eq!(include_data[5].0, "foo.h");
        assert!(!include_data[5].1, "foo.h should not be system");

        assert_eq!(include_data[6].0, "bar/baz.h");
        assert!(!include_data[6].1, "bar/baz.h should not be system");
    }

    #[test]
    fn test_includes_with_conditionals() {
        let mut parser = Parser::new().unwrap();
        let project = parser
            .parse_file(Path::new("tests/fixtures/includes_with_conditionals.c"))
            .unwrap();

        assert_eq!(project.files.len(), 1);
        let file = project.files.values().next().unwrap();

        let include_data: Vec<(&str, bool, _)> = file.iter_all_includes().collect();

        // Should parse ALL includes, including those inside #ifdef blocks
        // config.h, math.h, gobject/gvaluecollector.h, pango/pangocairo.h (in ifdef),
        // cogl/cogl.h, clutter-actor-private.h, clutter-actor-pango.h (in ifdef),
        // clutter-pango-private.h (in ifdef), clutter-action.h = 9 total
        assert_eq!(
            include_data.len(),
            9,
            "Expected 9 includes (including those in #ifdef), got {}",
            include_data.len()
        );

        // Verify we got the includes from inside #ifdef blocks
        let paths: Vec<&str> = include_data.iter().map(|(path, ..)| *path).collect();
        assert!(
            paths.contains(&"pango/pangocairo.h"),
            "Missing include from first #ifdef"
        );
        assert!(
            paths.contains(&"clutter/pango/clutter-actor-pango.h"),
            "Missing include from second #ifdef"
        );
        assert!(
            paths.contains(&"clutter/pango/clutter-pango-private.h"),
            "Missing include from second #ifdef"
        );
    }
}
