//! Saha functions and related types

use std::collections::HashMap;
use std::any::Any;

use crate::{
    ast::Ast,
    types::{
        Value, SahaType,
        objects::MemberVisibility
    },
    errors::{Error, RuntimeError},
    source::files::FilePosition,
    interpreter::AstVisitor
};

/// A result type for Saha callable `call`s. Returns either a Saha Value object
/// or a RuntimeError.
pub type SahaCallResult = Result<Value, RuntimeError>;

/// Collection of Saha function parameter definitions.
pub type SahaFunctionParamDefs = HashMap<String, FunctionParameter>;

/// A function call argument collection. Names mapped to Saha Values.
pub type SahaFunctionArguments = HashMap<String, Value>;

/// A single function parameter which Saha functions can accept.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParameter {
    pub name: String,
    pub param_type: Box<SahaType>,
    pub default: Value
}

/// Anything that needs to validate call arguments.
pub trait ValidatesArgs {
    /// Validate a collection of function/method call arguments.
    fn validate_args(&self, args: &SahaFunctionArguments, call_pos: &Option<FilePosition>) -> Result<SahaFunctionArguments, RuntimeError>;

    /// Validate args in case there is only a single parameter defined.
    fn validate_single_param_args(&self, args: &SahaFunctionArguments, call_pos: &Option<FilePosition>) -> Result<SahaFunctionArguments, RuntimeError>;
}

/// Anything which can be called in Saha. Functions and methods mainly.
pub trait SahaCallable: Send + Sync {
    /// Call this callable.
    fn call(&self, args: SahaFunctionArguments, return_type: Option<Box<SahaType>>, type_params: Vec<(char, SahaType)>, call_source_position: Option<FilePosition>) -> SahaCallResult;

    /// Get the parameters that this callable accepts.
    fn get_parameters(&self) -> SahaFunctionParamDefs;

    /// Get the return type this callable should return.
    fn get_return_type(&self) -> Box<SahaType>;

    /// Get the name of the function. Not necessarily the name with which it appears in source code.
    fn get_name(&self) -> String;

    /// Get the name of this function as it appears in source code.
    fn get_source_name(&self) -> String;

    /// Is this a static member callable?
    fn is_static(&self) -> bool;

    /// Is this a public member callable?
    fn is_public(&self) -> bool;

    /// Clone for boxed self.
    fn box_clone(&self) -> Box<dyn SahaCallable>;

    // internal
    fn as_any(&self) -> &dyn Any;

    // internal
    fn as_userfunction(&self) -> &UserFunction {
        let uf: &UserFunction = match self.as_any().downcast_ref::<UserFunction>() {
            Some(u) => u,
            None => panic!("Invalid internal callable cast, callable is not a user function")
        };

        return uf;
    }

    // internal
    fn as_corefunction(&self) -> &CoreFunction {
        let cf: &CoreFunction = match self.as_any().downcast_ref::<CoreFunction>() {
            Some(u) => u,
            None => panic!("Invalid internal callable cast, callable is not a core function")
        };

        return cf;
    }
}

impl Clone for Box<dyn SahaCallable> {
    fn clone(&self) -> Box<dyn SahaCallable> {
        return self.box_clone();
    }
}

/// Functions defined by the Saha core are CoreFunctions.
#[derive(Clone)]
pub struct CoreFunction {
    pub name: String,
    pub params: SahaFunctionParamDefs,
    pub return_type: Box<SahaType>,
    pub fn_ref: fn(args: SahaFunctionArguments) -> SahaCallResult,
    pub is_public: bool,
    pub is_static: bool
}

/// Functions defined by Saha developers in userland source code.
#[derive(Clone)]
pub struct UserFunction {
    pub source_name: String,
    pub name: String,
    pub params: SahaFunctionParamDefs,
    pub return_type: Box<SahaType>,
    pub ast: Ast,
    pub visibility: MemberVisibility,
    pub is_static: bool
}

impl SahaCallable for CoreFunction {
    fn call(&self, args: SahaFunctionArguments, return_type: Option<Box<SahaType>>, _type_params: Vec<(char, SahaType)>, call_source_position: Option<FilePosition>) -> SahaCallResult {
        let validated_args = self.params.validate_args(&args, &call_source_position)?;

        let ret_type = match &return_type {
            Some(t) => t.clone(),
            None => self.return_type.clone()
        };

        let res = (self.fn_ref)(validated_args.clone())?;

        match *res.kind {
            SahaType::Obj => {
                let wanted_name = match *ret_type {
                    SahaType::Name(ref n, ..) => n.clone(),
                    _ => {
                        let err = RuntimeError::new(
                            &format!(
                                "Return type mismatch for `{}`, expected `{:?}` but received `{:?}`",
                                self.name,
                                ret_type,
                                res.kind
                            ),
                            call_source_position
                        );

                        return Err(err);
                    }
                };

                // get the object type+implements list for comparing
                let mut inst_impl: Vec<String>;

                {
                    let st = crate::SAHA_SYMBOL_TABLE.lock().unwrap();
                    let inst_lockable = st.instances.get(&res.obj.unwrap()).unwrap();
                    let inst = inst_lockable.lock().unwrap();

                    inst_impl = inst.get_implements();
                    inst_impl.push(inst.get_fully_qualified_class_name());
                }

                if inst_impl.contains(&wanted_name) == false || res.kind != ret_type {
                    let err = RuntimeError::new(
                        &format!(
                            "Return type mismatch for `{}`, expected `{:?}` but received `{:?}`",
                            self.name,
                            ret_type,
                            res.kind
                        ),
                        call_source_position
                    );

                    return Err(err);
                }
            },
            _ => {
                if res.kind != ret_type {
                    let err = RuntimeError::new(
                        &format!(
                            "Return type mismatch for `{}`, expected `{:?}` but received `{:?}`",
                            self.name,
                            ret_type,
                            res.kind
                        ),
                        call_source_position
                    );

                    return Err(err);
                }
            }
        }

        return Ok(res);
    }

    fn get_parameters(&self) -> SahaFunctionParamDefs {
        return self.params.clone();
    }

    fn get_return_type(&self) -> Box<SahaType> {
        return self.return_type.clone()
    }

    fn get_name(&self) -> String {
        return self.name.to_owned();
    }

    fn get_source_name(&self) -> String {
        return self.get_name();
    }

    fn is_public(&self) -> bool {
        return self.is_public;
    }

    fn is_static(&self) -> bool {
        return self.is_static;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn box_clone(&self) -> Box<dyn SahaCallable> {
        return Box::new(self.clone());
    }
}

impl SahaCallable for UserFunction {
    fn call(&self, args: SahaFunctionArguments, return_type: Option<Box<SahaType>>, _type_params: Vec<(char, SahaType)>, call_source_position: Option<FilePosition>) -> SahaCallResult {
        let validated_args = self.params.validate_args(&args, &call_source_position)?;

        let ret_type = match &return_type {
            Some(t) => t.clone(),
            None => self.return_type.clone()
        };

        // clone the args to miminize possibility of side effects
        let mut ast_visitor = AstVisitor::new(&self.ast, validated_args.clone());

        let res = ast_visitor.start()?;

        match *res.kind {
            SahaType::Obj => {
                let wanted_name = match *ret_type {
                    SahaType::Name(ref n, ..) => n.clone(),
                    _ => {
                        let err = RuntimeError::new(
                            &format!(
                                "Return type mismatch for `{}`, expected `{:?}` but received `{:?}`",
                                self.name,
                                ret_type,
                                res.kind
                            ),
                            call_source_position
                        );

                        return Err(err);
                    }
                };

                // get the object type+implements list for comparing
                let mut inst_impl: Vec<String>;
                let inst_fqname: String;
                let inst_type: Box<SahaType>;

                {
                    let st = crate::SAHA_SYMBOL_TABLE.lock().unwrap();
                    let inst_lockable = st.instances.get(&res.obj.unwrap()).unwrap();
                    let inst = inst_lockable.lock().unwrap();

                    inst_impl = inst.get_implements();
                    inst_impl.push(inst.get_fully_qualified_class_name());
                    inst_fqname = inst.get_fully_qualified_class_name();
                    inst_type = inst.get_named_type();
                }

                if inst_impl.contains(&wanted_name) == false || ret_type != inst_type {
                    let err = RuntimeError::new(
                        &format!(
                            "Return type mismatch for `{}`, expected `{:?}` but received `{:?}`",
                            self.name,
                            ret_type,
                            inst_fqname
                        ),
                        call_source_position
                    );

                    return Err(err);
                }
            },
            _ => {
                if res.kind != ret_type {
                    let err = RuntimeError::new(
                        &format!(
                            "Return type mismatch for `{}`, expected `{:?}` but received `{:?}`",
                            self.name,
                            ret_type,
                            res.kind
                        ),
                        call_source_position
                    );

                    return Err(err);
                }
            }
        }

        return Ok(res);
    }

    fn get_parameters(&self) -> SahaFunctionParamDefs {
        return self.params.clone();
    }

    fn get_return_type(&self) -> Box<SahaType> {
        return self.return_type.clone();
    }

    fn get_name(&self) -> String {
        return self.name.to_owned();
    }

    fn get_source_name(&self) -> String {
        return self.source_name.to_owned();
    }

    fn is_public(&self) -> bool {
        match self.visibility {
            MemberVisibility::Private => false,
            _ => true
        }
    }

    fn is_static(&self) -> bool {
        return self.is_static;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn box_clone(&self) -> Box<dyn SahaCallable> {
        return Box::new(self.clone());
    }
}

impl ValidatesArgs for SahaFunctionParamDefs {
    /// Validate args in a situation where there is only a single parameter defined, which means
    /// we can call the function with no parameter name defined (to make code a little leaner).
    fn validate_single_param_args(&self, args: &SahaFunctionArguments, call_pos: &Option<FilePosition>) -> Result<SahaFunctionArguments, RuntimeError> {
        let mut validation_args: SahaFunctionArguments = args.clone();
        let mut validated_args = args.clone();

        if validation_args.contains_key("self") {
            // remove self or this single params validation will explode randomly as the arg order
            // might be different on each call
            validation_args.remove("self");
        }

        let param_name = self.keys().nth(0).unwrap();
        let param = self.values().nth(0).unwrap();
        let param_default = &param.default;
        let param_type = param.param_type.clone();

        if let SahaType::Void = *param_default.kind {
            if validation_args.is_empty() {
                // no arg and no default, which is a no-no
                let err = RuntimeError::new(
                    &format!("Invalid arguments, argument `{}` missing", param_name),
                    call_pos.to_owned()
                );

                return Err(err);
            }
        }

        let arg = validation_args.values().nth(0).unwrap();

        // arg type mismatch
        if *param_type != *arg {
            let err = RuntimeError::new(
                &format!(
                    "Invalid argument, `{}` is expected to be a `{}`, found `{}` instead",
                    param_name,
                    param_type.to_readable_string(),
                    arg.kind.to_readable_string()
                ),
                call_pos.to_owned()
            );

            return Err(err);
        }

        // all OK for this arg, just replace the given arg with a name in case the
        // given arg was an unnamed arg
        if validation_args.contains_key("") {
            validated_args.insert(param_name.clone(), validation_args.get("").unwrap().clone());
            validated_args.remove("");
        }

        return Ok(validated_args.clone());
    }

    fn validate_args(&self, args: &SahaFunctionArguments, call_pos: &Option<FilePosition>) -> Result<SahaFunctionArguments, RuntimeError> {
        if (args.len() == 1 && args.contains_key("self") == false) || (args.len() == 2 && args.contains_key("self")) {
            // if a function accepts only a single argument, we allow calling without setting a
            // parameter name (will use `""` internally)
            return self.validate_single_param_args(&args, call_pos);
        }

        for (name, ref param) in self {
            let param_type = param.param_type.clone();
            let param_default = param.default.to_owned();

            // arg missing, see if default is provided
            if args.contains_key(name) == false {
                match *param_default.kind {
                    SahaType::Void => {
                        let err = RuntimeError::new(
                            &format!("Invalid arguments, argument `{}` missing", name),
                            call_pos.to_owned()
                        );

                        return Err(err);
                    }
                    _ => ()
                };
            }

            let arg = args.get(name).unwrap();

            // arg type mismatch
            if *param_type != *arg {
                let err = RuntimeError::new(
                    &format!(
                        "Invalid argument, `{}` is expected to be a `{}`, found `{}` instead",
                        name,
                        param_type.to_readable_string(),
                        arg.kind.to_readable_string()
                    ),
                    call_pos.to_owned()
                );

                return Err(err);
            }

            // all OK for this arg, continue loop
        }

        return Ok(args.clone());
    }
}
