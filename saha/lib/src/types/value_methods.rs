//! Saha value methods
//!
//! Methods that are bound to values that are not object instances.

use noisy_float::prelude::*;

use std::collections::HashMap;

use crate::types::{
    Value,
    functions::{SahaFunctionParamDefs, SahaFunctionArguments, SahaCallResult}
};

pub type ValueMethodFn = fn(caller: Value, args: SahaFunctionArguments) -> SahaCallResult;

/// Get value methods that are tied to `str` values.
pub fn get_str_methods() -> HashMap<String, (SahaFunctionParamDefs, ValueMethodFn)> {
    return HashMap::new();
}

/// Get value methods that are tied to `int` values.
pub fn get_int_methods() -> HashMap<String, (SahaFunctionParamDefs, ValueMethodFn)> {
    let mut fns: HashMap<String, (SahaFunctionParamDefs, ValueMethodFn)> = HashMap::new();

    fns.insert("toString".to_string(), (HashMap::new(), int_to_string));
    fns.insert("toFloat".to_string(), (HashMap::new(), int_to_float));

    return fns;
}

pub fn get_float_methods() -> HashMap<String, (SahaFunctionParamDefs, ValueMethodFn)> {
    let mut fns: HashMap<String, (SahaFunctionParamDefs, ValueMethodFn)> = HashMap::new();

    fns.insert("toString".to_string(), (HashMap::new(), float_to_string));

    return fns;
}

/// Convert `int` to `str`.
pub fn int_to_string(caller: Value, _: SahaFunctionArguments) -> SahaCallResult {
    let as_string = caller.int.unwrap().to_string();

    return Ok(Value::str(as_string));
}

/// Convert `int` to `float`.
pub fn int_to_float(caller: Value, _: SahaFunctionArguments) -> SahaCallResult {
    let intvalue = caller.int.unwrap();

    let floatvalue: f64 = intvalue as f64;

    return Ok(Value::float(r64(floatvalue)));
}

/// Convert `float` to `str`.
pub fn float_to_string(caller: Value, _: SahaFunctionArguments) -> SahaCallResult {
    let as_string = caller.float.unwrap().to_string();

    return Ok(Value::str(as_string));
}