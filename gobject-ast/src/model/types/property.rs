use serde::{Deserialize, Serialize};

use crate::model::{
    SourceLocation,
    expression::{Argument, CallExpression, Expression},
    operators::{BinaryOp, UnaryOp},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParamFlag {
    /// The parameter is readable (value: 1)
    Readable,
    /// The parameter is writable (value: 2)
    Writable,
    /// Alias for READABLE | WRITABLE (value: 3)
    ReadWrite,
    /// The parameter will be set upon object construction (value: 4)
    Construct,
    /// The parameter can only be set upon object construction (value: 8)
    ConstructOnly,
    /// Strict validation not required upon parameter conversion (value: 16)
    LaxValidation,
    /// String used as name is guaranteed to remain valid (value: 32)
    StaticName,
    /// Internal flag (value: 32)
    Private,
    /// String used as nick is guaranteed to remain valid (value: 64)
    StaticNick,
    /// String used as blurb is guaranteed to remain valid (value: 128)
    StaticBlurb,
    /// Alias for STATIC_NAME | STATIC_NICK | STATIC_BLURB
    StaticStrings,
    /// No automatic notify signal emission (value: 1073741824)
    ExplicitNotify,
    /// The parameter is deprecated (value: 2147483648)
    Deprecated,
    /// Custom or unrecognized flag
    Unknown(String),
}

impl ParamFlag {
    pub fn from_identifier(name: &str) -> Self {
        match name {
            "G_PARAM_READABLE" => ParamFlag::Readable,
            "G_PARAM_WRITABLE" => ParamFlag::Writable,
            "G_PARAM_READWRITE" => ParamFlag::ReadWrite,
            "G_PARAM_CONSTRUCT" => ParamFlag::Construct,
            "G_PARAM_CONSTRUCT_ONLY" => ParamFlag::ConstructOnly,
            "G_PARAM_LAX_VALIDATION" => ParamFlag::LaxValidation,
            "G_PARAM_STATIC_NAME" => ParamFlag::StaticName,
            "G_PARAM_PRIVATE" => ParamFlag::Private,
            "G_PARAM_STATIC_NICK" => ParamFlag::StaticNick,
            "G_PARAM_STATIC_BLURB" => ParamFlag::StaticBlurb,
            "G_PARAM_STATIC_STRINGS" => ParamFlag::StaticStrings,
            "G_PARAM_EXPLICIT_NOTIFY" => ParamFlag::ExplicitNotify,
            "G_PARAM_DEPRECATED" => ParamFlag::Deprecated,
            _ => ParamFlag::Unknown(name.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            ParamFlag::Readable => "G_PARAM_READABLE",
            ParamFlag::Writable => "G_PARAM_WRITABLE",
            ParamFlag::ReadWrite => "G_PARAM_READWRITE",
            ParamFlag::Construct => "G_PARAM_CONSTRUCT",
            ParamFlag::ConstructOnly => "G_PARAM_CONSTRUCT_ONLY",
            ParamFlag::LaxValidation => "G_PARAM_LAX_VALIDATION",
            ParamFlag::StaticName => "G_PARAM_STATIC_NAME",
            ParamFlag::Private => "G_PARAM_PRIVATE",
            ParamFlag::StaticNick => "G_PARAM_STATIC_NICK",
            ParamFlag::StaticBlurb => "G_PARAM_STATIC_BLURB",
            ParamFlag::StaticStrings => "G_PARAM_STATIC_STRINGS",
            ParamFlag::ExplicitNotify => "G_PARAM_EXPLICIT_NOTIFY",
            ParamFlag::Deprecated => "G_PARAM_DEPRECATED",
            ParamFlag::Unknown(name) => name.as_str(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    pub name: String,
    pub property_type: PropertyType,
    pub nick: Option<String>,
    pub blurb: Option<String>,
    pub flags: Vec<ParamFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyType {
    String,
    Int { min: i64, max: i64, default: i64 },
    UInt { min: u64, max: u64, default: u64 },
    Boolean { default: bool },
    Float { min: f64, max: f64, default: f64 },
    Double { min: f64, max: f64, default: f64 },
    Enum { enum_type: String, default: i64 },
    Flags { flags_type: String, default: u64 },
    Object { object_type: String },
    Boxed { boxed_type: String },
    Pointer,
    GType { is_a_type: String },
    Variant,
    Override,
    Unknown { spec_function: String },
}

impl Property {
    /// Extract property from g_param_spec_* call
    /// Call signature varies by type:
    /// - g_param_spec_string(name, nick, blurb, default, flags)
    /// - g_param_spec_int(name, nick, blurb, min, max, default, flags)
    /// - g_param_spec_object(name, nick, blurb, object_type, flags)
    pub fn from_param_spec_call(call: &CallExpression) -> Option<Self> {
        let func_name = call.function_name_str()?;

        // Extract common arguments (name, nick, blurb)
        let args = &call.arguments;
        if args.len() < 3 {
            return None;
        }

        let name = extract_string_arg(&args[0])?;
        let nick = extract_string_arg(&args[1]);
        let blurb = extract_string_arg(&args[2]);

        let property_type = match func_name {
            "g_param_spec_string" => {
                // (name, nick, blurb, default, flags)
                PropertyType::String
            }
            "g_param_spec_boolean" => {
                // (name, nick, blurb, default, flags)
                let default = if args.len() > 3 {
                    extract_boolean_arg(&args[3]).unwrap_or(false)
                } else {
                    false
                };
                PropertyType::Boolean { default }
            }
            "g_param_spec_int" => {
                // (name, nick, blurb, min, max, default, flags)
                let min = if args.len() > 3 {
                    extract_int_arg(&args[3]).unwrap_or(i64::MIN)
                } else {
                    i64::MIN
                };
                let max = if args.len() > 4 {
                    extract_int_arg(&args[4]).unwrap_or(i64::MAX)
                } else {
                    i64::MAX
                };
                let default = if args.len() > 5 {
                    extract_int_arg(&args[5]).unwrap_or(0)
                } else {
                    0
                };
                PropertyType::Int { min, max, default }
            }
            "g_param_spec_uint" => {
                // (name, nick, blurb, min, max, default, flags)
                let min = if args.len() > 3 {
                    extract_uint_arg(&args[3]).unwrap_or(0)
                } else {
                    0
                };
                let max = if args.len() > 4 {
                    extract_uint_arg(&args[4]).unwrap_or(u64::MAX)
                } else {
                    u64::MAX
                };
                let default = if args.len() > 5 {
                    extract_uint_arg(&args[5]).unwrap_or(0)
                } else {
                    0
                };
                PropertyType::UInt { min, max, default }
            }
            "g_param_spec_float" => {
                // (name, nick, blurb, min, max, default, flags)
                let min = if args.len() > 3 {
                    extract_float_arg(&args[3]).unwrap_or(f64::MIN)
                } else {
                    f64::MIN
                };
                let max = if args.len() > 4 {
                    extract_float_arg(&args[4]).unwrap_or(f64::MAX)
                } else {
                    f64::MAX
                };
                let default = if args.len() > 5 {
                    extract_float_arg(&args[5]).unwrap_or(0.0)
                } else {
                    0.0
                };
                PropertyType::Float { min, max, default }
            }
            "g_param_spec_double" => {
                // (name, nick, blurb, min, max, default, flags)
                let min = if args.len() > 3 {
                    extract_float_arg(&args[3]).unwrap_or(f64::MIN)
                } else {
                    f64::MIN
                };
                let max = if args.len() > 4 {
                    extract_float_arg(&args[4]).unwrap_or(f64::MAX)
                } else {
                    f64::MAX
                };
                let default = if args.len() > 5 {
                    extract_float_arg(&args[5]).unwrap_or(0.0)
                } else {
                    0.0
                };
                PropertyType::Double { min, max, default }
            }
            "g_param_spec_enum" => {
                // (name, nick, blurb, enum_type, default, flags)
                let enum_type = if args.len() > 3 {
                    extract_identifier_arg(&args[3]).unwrap_or_default()
                } else {
                    String::new()
                };
                let default = if args.len() > 4 {
                    extract_int_arg(&args[4]).unwrap_or(0)
                } else {
                    0
                };
                PropertyType::Enum { enum_type, default }
            }
            "g_param_spec_flags" => {
                // (name, nick, blurb, flags_type, default, flags)
                let flags_type = if args.len() > 3 {
                    extract_identifier_arg(&args[3]).unwrap_or_default()
                } else {
                    String::new()
                };
                let default = if args.len() > 4 {
                    extract_uint_arg(&args[4]).unwrap_or(0)
                } else {
                    0
                };
                PropertyType::Flags {
                    flags_type,
                    default,
                }
            }
            "g_param_spec_object" => {
                // (name, nick, blurb, object_type, flags)
                let object_type = if args.len() > 3 {
                    extract_identifier_arg(&args[3]).unwrap_or_default()
                } else {
                    String::new()
                };
                PropertyType::Object { object_type }
            }
            "g_param_spec_boxed" => {
                // (name, nick, blurb, boxed_type, flags)
                let boxed_type = if args.len() > 3 {
                    extract_identifier_arg(&args[3]).unwrap_or_default()
                } else {
                    String::new()
                };
                PropertyType::Boxed { boxed_type }
            }
            "g_param_spec_pointer" => PropertyType::Pointer,
            "g_param_spec_gtype" => {
                // (name, nick, blurb, is_a_type, flags)
                let is_a_type = if args.len() > 3 {
                    extract_identifier_arg(&args[3]).unwrap_or_default()
                } else {
                    String::new()
                };
                PropertyType::GType { is_a_type }
            }
            "g_param_spec_variant" => PropertyType::Variant,
            _ => PropertyType::Unknown {
                spec_function: func_name.to_string(),
            },
        };

        // Extract flags (usually last argument)
        let flags = if let Some(last_arg) = args.last() {
            extract_flags_arg(last_arg)
        } else {
            Vec::new()
        };

        Some(Property {
            name,
            property_type,
            nick,
            blurb,
            flags,
        })
    }

    /// Extract property from g_object_class_override_property call
    /// Call signature: g_object_class_override_property(oclass, property_id,
    /// name)
    pub fn from_override_property_call(call: &CallExpression) -> Option<Self> {
        let func_name = call.function_name_str()?;
        if func_name != "g_object_class_override_property" {
            return None;
        }

        let args = &call.arguments;
        if args.len() < 3 {
            return None;
        }

        // Third argument is the property name
        let name = extract_string_arg(&args[2])?;

        Some(Property {
            name,
            property_type: PropertyType::Override,
            nick: None,
            blurb: None,
            flags: Vec::new(),
        })
    }
}

// Helper functions to extract values from expression arguments

fn extract_string_arg(arg: &Argument) -> Option<String> {
    match arg {
        Argument::Expression(boxed_expr) => match &**boxed_expr {
            Expression::StringLiteral(s) => {
                // Remove quotes
                let text = s.value.trim_matches('"');
                Some(text.to_owned())
            }
            Expression::Null(_) => None,
            _ => None,
        },
    }
}

fn extract_int_arg(arg: &Argument) -> Option<i64> {
    match arg {
        Argument::Expression(boxed_expr) => match &**boxed_expr {
            Expression::NumberLiteral(num) => num.value.parse().ok(),
            Expression::Unary(unary) => {
                // Handle negative numbers like -1
                if matches!(unary.operator, UnaryOp::Negate) {
                    if let Expression::NumberLiteral(num) = &*unary.operand {
                        return num.value.parse::<i64>().ok().map(|v| -v);
                    }
                }
                None
            }
            _ => None,
        },
    }
}

fn extract_uint_arg(arg: &Argument) -> Option<u64> {
    match arg {
        Argument::Expression(boxed_expr) => match &**boxed_expr {
            Expression::NumberLiteral(num) => num.value.parse().ok(),
            _ => None,
        },
    }
}

fn extract_float_arg(arg: &Argument) -> Option<f64> {
    match arg {
        Argument::Expression(boxed_expr) => match &**boxed_expr {
            Expression::NumberLiteral(num) => num.value.parse().ok(),
            Expression::Unary(unary) => {
                if matches!(unary.operator, UnaryOp::Negate) {
                    if let Expression::NumberLiteral(num) = &*unary.operand {
                        return num.value.parse::<f64>().ok().map(|v| -v);
                    }
                }
                None
            }
            _ => None,
        },
    }
}

fn extract_boolean_arg(arg: &Argument) -> Option<bool> {
    match arg {
        Argument::Expression(boxed_expr) => match &**boxed_expr {
            Expression::Boolean(b) => Some(b.value),
            Expression::Identifier(id) => {
                // TRUE/FALSE macros
                match id.name.as_str() {
                    "TRUE" | "true" => Some(true),
                    "FALSE" | "false" => Some(false),
                    _ => None,
                }
            }
            _ => None,
        },
    }
}

fn extract_identifier_arg(arg: &Argument) -> Option<String> {
    match arg {
        Argument::Expression(boxed_expr) => match &**boxed_expr {
            Expression::Identifier(id) => Some(id.name.clone()),
            Expression::Call(call) => {
                // Handle macros like G_TYPE_STRING
                Some(call.function_name())
            }
            _ => None,
        },
    }
}

fn extract_flags_arg(arg: &Argument) -> Vec<ParamFlag> {
    let mut flags = Vec::new();

    fn collect_flags(expr: &Expression, flags: &mut Vec<ParamFlag>) {
        match expr {
            Expression::Identifier(id) => {
                // Convert any identifier to ParamFlag
                flags.push(ParamFlag::from_identifier(&id.name));
            }
            Expression::Binary(binary) => {
                // Handle flags combined with | operator
                if matches!(binary.operator, BinaryOp::BitwiseOr) {
                    collect_flags(&*binary.left, flags);
                    collect_flags(&*binary.right, flags);
                }
            }
            _ => {}
        }
    }

    match arg {
        Argument::Expression(boxed_expr) => {
            collect_flags(&**boxed_expr, &mut flags);
        }
    }

    flags
}

/// Information about a param_spec assignment found in a class_init function
#[derive(Debug, Clone)]
pub enum ParamSpecAssignment {
    /// Array subscript pattern: props[PROP_X] = g_param_spec_*()
    ArraySubscript {
        array_name: String,
        enum_value: String,
        property_name: String,
        statement_location: SourceLocation,
        call: CallExpression,
    },
    /// Variable pattern: param_spec = g_param_spec_*()
    Variable {
        variable_name: String,
        property_name: String,
        statement_location: SourceLocation,
        call: CallExpression,
    },
}
