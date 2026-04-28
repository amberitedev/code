use deno_core::{
    FastString, ModuleLoadResponse, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType,
    RequestedModuleType, ResolutionKind,
};
use deno_error::JsErrorBox;

/// Loads TypeScript/JavaScript macro files from the local filesystem,
/// transpiling `.ts` files to JavaScript via deno_ast.
pub struct TypescriptModuleLoader;

impl deno_core::ModuleLoader for TypescriptModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, JsErrorBox> {
        let referrer_url = ModuleSpecifier::parse(referrer).unwrap_or_else(|_| {
            ModuleSpecifier::parse("file:///").expect("fallback url always valid")
        });
        deno_core::resolve_import(specifier, referrer_url.as_str())
            .map_err(|e| JsErrorBox::generic(e.to_string()))
    }

    fn load(
        &self,
        specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        _requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        let specifier = specifier.clone();
        ModuleLoadResponse::Async(Box::pin(async move {
            let path = specifier
                .to_file_path()
                .map_err(|_| JsErrorBox::generic(format!("not a file URL: {specifier}")))?;

            let source = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| JsErrorBox::generic(e.to_string()))?;

            let is_ts = path.extension().map_or(false, |e| e == "ts");
            let js_source = if is_ts {
                transpile_ts(&specifier, source)
                    .map_err(|e| JsErrorBox::generic(e.to_string()))?
            } else {
                source
            };

            Ok(ModuleSource::new(
                ModuleType::JavaScript,
                ModuleSourceCode::String(FastString::from(js_source)),
                &specifier,
                None,
            ))
        }))
    }
}

fn transpile_ts(
    specifier: &ModuleSpecifier,
    source: String,
) -> Result<String, deno_core::error::AnyError> {
    let parsed = deno_ast::parse_module(deno_ast::ParseParams {
        specifier: specifier.clone(),
        text: source.into(),
        media_type: deno_ast::MediaType::TypeScript,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
    })?;
    let result = parsed.transpile(
        &deno_ast::TranspileOptions::default(),
        &deno_ast::EmitOptions::default(),
    )?;
    Ok(String::from_utf8(result.into_source().source).unwrap_or_default())
}
