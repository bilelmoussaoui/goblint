use std::path::Path;

use gobject_ast::{model::types::GObjectTypeKind, parser::Parser};

fn parse_fixture(fixture_name: &str) -> gobject_ast::model::Project {
    let fixture_path = Path::new("tests/fixtures/gobject").join(fixture_name);
    let mut parser = Parser::new().expect("Failed to create parser");
    parser.parse_file(&fixture_path).expect("Failed to parse")
}

#[test]
fn test_g_declare_final_type() {
    let project = parse_fixture("declare_final.h");
    let file = project.files.values().next().expect("No files parsed");

    assert_eq!(file.gobject_types.len(), 1);
    let gobj = &file.gobject_types[0];

    assert_eq!(gobj.type_name, "MyWidget");
    assert_eq!(gobj.type_macro, "MY_TYPE_WIDGET");

    match &gobj.kind {
        GObjectTypeKind::DeclareFinal {
            function_prefix,
            module_prefix,
            type_prefix,
            parent_type,
        } => {
            assert_eq!(function_prefix, "my_widget");
            assert_eq!(module_prefix, "MY");
            assert_eq!(type_prefix, "WIDGET");
            assert_eq!(parent_type, "GtkWidget");
        }
        _ => panic!("Expected DeclareFinal, got {:?}", gobj.kind),
    }
}

#[test]
fn test_g_declare_derivable_type() {
    let project = parse_fixture("declare_derivable.h");
    let file = project.files.values().next().expect("No files parsed");

    assert_eq!(file.gobject_types.len(), 1);
    let gobj = &file.gobject_types[0];

    assert_eq!(gobj.type_name, "MyObject");
    assert_eq!(gobj.type_macro, "MY_TYPE_OBJECT");

    match &gobj.kind {
        GObjectTypeKind::DeclareDerivable {
            function_prefix,
            module_prefix,
            type_prefix,
            parent_type,
        } => {
            assert_eq!(function_prefix, "my_object");
            assert_eq!(module_prefix, "MY");
            assert_eq!(type_prefix, "OBJECT");
            assert_eq!(parent_type, "GObject");
        }
        _ => panic!("Expected DeclareDerivable, got {:?}", gobj.kind),
    }
}

#[test]
fn test_g_declare_interface() {
    let project = parse_fixture("declare_interface.h");
    let file = project.files.values().next().expect("No files parsed");

    assert_eq!(file.gobject_types.len(), 1);
    let gobj = &file.gobject_types[0];

    assert_eq!(gobj.type_name, "MyInterface");
    assert_eq!(gobj.type_macro, "MY_TYPE_INTERFACE");

    match &gobj.kind {
        GObjectTypeKind::DeclareInterface {
            function_prefix,
            module_prefix,
            type_prefix,
            prerequisite_type,
        } => {
            assert_eq!(function_prefix, "my_interface");
            assert_eq!(module_prefix, "MY");
            assert_eq!(type_prefix, "INTERFACE");
            assert_eq!(prerequisite_type, "GObject");
        }
        _ => panic!("Expected DeclareInterface, got {:?}", gobj.kind),
    }
}

#[test]
fn test_g_define_type() {
    let project = parse_fixture("define_type.c");
    let file = project.files.values().next().expect("No files parsed");

    assert_eq!(file.gobject_types.len(), 1);
    let gobj = &file.gobject_types[0];

    assert_eq!(gobj.type_name, "MyWidget");
    assert_eq!(gobj.type_macro, "TYPE_MYWIDGET");

    match &gobj.kind {
        GObjectTypeKind::DefineType {
            function_prefix,
            parent_type,
        } => {
            assert_eq!(function_prefix, "my_widget");
            assert_eq!(parent_type, "GTK_TYPE_WIDGET");
        }
        _ => panic!("Expected DefineType, got {:?}", gobj.kind),
    }
}

#[test]
fn test_g_define_type_with_private() {
    let project = parse_fixture("define_type_with_private.c");
    let file = project.files.values().next().expect("No files parsed");

    assert_eq!(file.gobject_types.len(), 1);
    let gobj = &file.gobject_types[0];

    assert_eq!(gobj.type_name, "MyObject");

    match &gobj.kind {
        GObjectTypeKind::DefineTypeWithPrivate {
            function_prefix,
            parent_type,
        } => {
            assert_eq!(function_prefix, "my_object");
            assert_eq!(parent_type, "G_TYPE_OBJECT");
        }
        _ => panic!("Expected DefineTypeWithPrivate, got {:?}", gobj.kind),
    }
}

#[test]
fn test_class_struct_with_vfuncs() {
    let project = parse_fixture("class_with_vfuncs.h");
    let file = project.files.values().next().expect("No files parsed");

    assert_eq!(file.gobject_types.len(), 1);
    let gobj = &file.gobject_types[0];

    assert_eq!(gobj.type_name, "MyObject");

    let class_struct = gobj.class_struct.as_ref().expect("No class struct parsed");

    assert_eq!(class_struct.name, "_MyObjectClass");
    assert!(
        class_struct.vfuncs.len() >= 2,
        "Expected at least 2 vfuncs, got {}",
        class_struct.vfuncs.len()
    );

    // Check for specific vfuncs
    let vfunc_names: Vec<_> = class_struct.vfuncs.iter().map(|v| &v.name).collect();
    assert!(
        vfunc_names.contains(&&"do_something".to_string()),
        "Missing vfunc 'do_something'"
    );
    assert!(
        vfunc_names.contains(&&"get_value".to_string()),
        "Missing vfunc 'get_value'"
    );
}

#[test]
fn test_multiple_gobject_types_in_one_file() {
    let project = parse_fixture("multiple_types.h");
    let file = project.files.values().next().expect("No files parsed");

    assert!(
        file.gobject_types.len() >= 2,
        "Expected at least 2 GObject types, got {}",
        file.gobject_types.len()
    );

    let type_names: Vec<_> = file.gobject_types.iter().map(|g| &g.type_name).collect();
    assert!(type_names.contains(&&"MyWidget".to_string()));
    assert!(type_names.contains(&&"MyInterface".to_string()));
}

#[test]
fn test_vfunc_parameters_and_return_types() {
    let project = parse_fixture("class_with_vfuncs.h");
    let file = project.files.values().next().expect("No files parsed");

    let gobj = &file.gobject_types[0];
    let class_struct = gobj.class_struct.as_ref().expect("No class struct");

    // Find the do_something vfunc
    let do_something = class_struct
        .vfuncs
        .iter()
        .find(|v| v.name == "do_something")
        .expect("Missing do_something vfunc");

    // Check return type
    assert_eq!(do_something.return_type, Some("void".to_string()));

    // Check parameters
    assert_eq!(do_something.parameters.len(), 2);
    assert_eq!(do_something.parameters[0].type_name, "MyObject*");
    assert_eq!(do_something.parameters[0].name, Some("self".to_string()));
    assert_eq!(do_something.parameters[1].type_name, "int");
    assert_eq!(do_something.parameters[1].name, Some("value".to_string()));

    // Find the get_value vfunc
    let get_value = class_struct
        .vfuncs
        .iter()
        .find(|v| v.name == "get_value")
        .expect("Missing get_value vfunc");

    // Check return type
    assert_eq!(get_value.return_type, Some("int".to_string()));

    // Check parameters
    assert_eq!(get_value.parameters.len(), 1);
    assert_eq!(get_value.parameters[0].type_name, "MyObject*");
}

#[test]
fn test_property_extraction() {
    let project = parse_fixture("properties.c");
    let file = project.files.values().next().expect("No files parsed");

    // Check that we parse the GObject type definition
    assert_eq!(file.gobject_types.len(), 1);
    let gobject_type = &file.gobject_types[0];

    // Get the class_init function name
    let class_init_name = gobject_type.class_init_function_name();
    assert_eq!(class_init_name, "my_object_class_init");

    // Find the class_init function
    let class_init = file
        .functions
        .iter()
        .find(|f| f.name == class_init_name)
        .expect("class_init function not found");

    // Extract properties
    let properties = gobject_type.extract_properties(class_init);

    // Should have extracted 2 properties: name and value
    assert!(
        properties.len() >= 2,
        "Expected at least 2 properties, got {}",
        properties.len()
    );

    // Find the "name" property
    let name_prop = properties.iter().find(|p| p.name == "name");
    assert!(name_prop.is_some(), "Property 'name' not found");
    let name_prop = name_prop.unwrap();

    use gobject_ast::model::types::{ParamFlag, PropertyType};
    assert!(matches!(name_prop.property_type, PropertyType::String));
    assert_eq!(name_prop.nick, Some("Name".to_string()));
    assert_eq!(name_prop.blurb, Some("The object name".to_string()));
    assert!(name_prop.flags.contains(&ParamFlag::ReadWrite));

    // Find the "value" property
    let value_prop = properties.iter().find(|p| p.name == "value");
    assert!(value_prop.is_some(), "Property 'value' not found");
    let value_prop = value_prop.unwrap();

    match &value_prop.property_type {
        PropertyType::Int { min, max, default } => {
            assert_eq!(*min, 0);
            assert_eq!(*max, 100);
            assert_eq!(*default, 0);
        }
        _ => panic!("Expected Int property type"),
    }
    assert_eq!(value_prop.nick, Some("Value".to_string()));
    assert!(value_prop.flags.contains(&ParamFlag::ReadWrite));
}

#[test]
fn test_property_installation() {
    let project = parse_fixture("properties.c");
    let file = project.files.values().next().expect("No files parsed");

    // Find class_init function
    let class_init = file
        .functions
        .iter()
        .find(|f| f.name == "my_object_class_init")
        .expect("No class_init found");

    // Check that it calls g_param_spec_* functions
    let param_spec_calls = class_init.find_calls(&["g_param_spec_string", "g_param_spec_int"]);
    assert!(
        param_spec_calls.len() >= 2,
        "Expected at least 2 g_param_spec calls, got {}",
        param_spec_calls.len()
    );

    // Check that it calls g_object_class_install_properties
    let install_calls = class_init.find_calls(&["g_object_class_install_properties"]);
    assert!(
        !install_calls.is_empty(),
        "Expected g_object_class_install_properties call"
    );
}

#[test]
fn test_signals_enum() {
    let project = parse_fixture("signals.c");
    let file = project.files.values().next().expect("No files parsed");

    // Check that we have the signals enum
    let signal_enum = file
        .enums
        .iter()
        .find(|e| e.values.iter().any(|v| v.name == "SIGNAL_CHANGED"));
    assert!(signal_enum.is_some(), "Signal enum not found");

    let signal_enum = signal_enum.unwrap();
    assert!(
        signal_enum
            .values
            .iter()
            .any(|v| v.name == "SIGNAL_CHANGED")
    );
    assert!(
        signal_enum
            .values
            .iter()
            .any(|v| v.name == "SIGNAL_ACTIVATED")
    );
    assert!(signal_enum.values.iter().any(|v| v.name == "N_SIGNALS"));
}

#[test]
fn test_signal_creation() {
    let project = parse_fixture("signals.c");
    let file = project.files.values().next().expect("No files parsed");

    let class_init = file
        .functions
        .iter()
        .find(|f| f.name == "my_object_class_init")
        .expect("No class_init found");

    // Check that it calls g_signal_new
    let signal_new_calls = class_init.find_calls(&["g_signal_new"]);
    assert!(
        signal_new_calls.len() >= 2,
        "Expected at least 2 g_signal_new calls, got {}",
        signal_new_calls.len()
    );
}

#[test]
fn test_interface_implementation() {
    let project = parse_fixture("interface_impl.c");
    let file = project.files.values().next().expect("No files parsed");

    // Should have a G_DEFINE_TYPE_WITH_CODE macro
    assert_eq!(file.gobject_types.len(), 1);

    // Should have the interface init function
    let iface_init = file
        .functions
        .iter()
        .find(|f| f.name == "my_interface_init")
        .expect("No interface init found");

    assert!(iface_init.is_definition);
}

#[test]
fn test_boxed_type() {
    let project = parse_fixture("boxed_types.c");
    let file = project.files.values().next().expect("No files parsed");

    // Check that we found the boxed type definition
    // G_DEFINE_BOXED_TYPE should be parsed
    assert!(!file.gobject_types.is_empty(), "No GObject types found");

    // Should have copy and free functions
    assert!(file.functions.iter().any(|f| f.name == "my_struct_copy"));
    assert!(file.functions.iter().any(|f| f.name == "my_struct_free"));
}

#[test]
fn test_gtk_doc_comments() {
    let project = parse_fixture("annotations.h");
    let file = project.files.values().next().expect("No files parsed");

    // Should have the declared type
    assert_eq!(file.gobject_types.len(), 1);
    assert_eq!(file.gobject_types[0].type_name, "MyObject");

    // Should have all the documented functions
    assert!(file.functions.iter().any(|f| f.name == "my_object_new"));
    assert!(
        file.functions
            .iter()
            .any(|f| f.name == "my_object_set_name")
    );
    assert!(
        file.functions
            .iter()
            .any(|f| f.name == "my_object_get_children")
    );
    assert!(file.functions.iter().any(|f| f.name == "my_object_process"));
}

#[test]
fn test_custom_param_spec() {
    let project = parse_fixture("custom_param_spec.c");
    let file = project.files.values().next().expect("No files parsed");

    assert_eq!(file.gobject_types.len(), 1);
    let gobject_type = &file.gobject_types[0];

    let class_init_name = gobject_type.class_init_function_name();
    let class_init = file
        .functions
        .iter()
        .find(|f| f.name == class_init_name)
        .expect("class_init function not found");

    let properties = gobject_type.extract_properties(class_init);

    // Should have extracted the custom color property
    assert_eq!(properties.len(), 1, "Expected 1 property");

    let color_prop = &properties[0];
    assert_eq!(color_prop.name, "color");
    assert_eq!(color_prop.nick, Some("Color".to_string()));
    assert_eq!(color_prop.blurb, Some("The object color".to_string()));

    // Custom param specs should be captured as Unknown
    use gobject_ast::model::types::{ParamFlag, PropertyType};
    match &color_prop.property_type {
        PropertyType::Unknown { spec_function } => {
            assert_eq!(spec_function, "cogl_param_spec_color");
        }
        _ => panic!(
            "Expected Unknown property type, got {:?}",
            color_prop.property_type
        ),
    }

    assert!(color_prop.flags.contains(&ParamFlag::ReadWrite));
    assert!(color_prop.flags.contains(&ParamFlag::StaticStrings));
}
