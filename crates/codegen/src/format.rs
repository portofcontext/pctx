use biome_formatter::IndentStyle;
use biome_js_formatter::{context::JsFormatOptions, format_node};
use biome_js_parser::{JsParserOptions, parse};
use biome_js_syntax::JsFileSource;

fn format_code(code: &str, source: JsFileSource) -> String {
    let parsed = parse(code, source, JsParserOptions::default());
    let options = JsFormatOptions::new(source).with_indent_style(IndentStyle::Space);

    let formatted = format_node(options, &parsed.syntax()).map(|f| f.print());
    if let Ok(Ok(printed)) = formatted {
        printed.into_code()
    } else {
        code.into()
    }
}

pub fn format_ts(code: &str) -> String {
    format_code(code, JsFileSource::ts())
}

pub fn format_d_ts(code: &str) -> String {
    format_code(code, JsFileSource::d_ts())
}

pub fn format_js(code: &str) -> String {
    format_code(code, JsFileSource::js_script())
}
