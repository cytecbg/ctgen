pub const CONFIG_DIR_NAME: &str = "ctgen";
pub const CONFIG_FILE_NAME: &str = "Profiles.toml";
pub const CONFIG_NAME_DEFAULT: &str = "default";
pub const CONFIG_NAME_PATTERN: &str = r"^[a-zA-Z-_]+$";

pub const PROFILE_DEFAULT_FILENAME: &str = "Ctgen.toml";

pub const FILE_EXT_RHAI: &str = ".rhai";

pub const DUMMY_TEMPLATE: &str = r#"
# Context Test

Table {{table/name}} is selected for this task run. The dummy prompt has been answered with {{#if (eq prompts/dummy "1")}}YES{{else}}NO{{/if}}.

Table primary key: {{#concat table/primary_key separator=", " render_all=true}}`{{this}}`{{/concat}}

Table columns: {{#concat table/columns separator=", " render_all=true}}`{{name}}`{{/concat}}

# Inflector Test

{{inflect table/name to_camel_case=true}}
{{inflect table/name to_pascal_case=true}}
{{inflect table/name to_snake_case=true}}
{{inflect table/name to_screaming_snake_case=true}}
{{inflect table/name to_kebab_case=true}}
{{inflect table/name to_sentence_case=true}}
{{inflect table/name to_title_case=true}}
{{inflect table/name to_foreign_key=true}}
{{inflect table/name to_class_case=true}}
{{inflect table/name to_table_case=true}}
{{inflect table/name to_plural=true}}
{{inflect table/name to_singular=true}}
{{inflect table/name to_upper_case=true}}
{{inflect table/name to_lower_case=true}}

# Concat Test

{{concat table/columns separator=", "}}

# Raw Context Dump

{{{json this}}}
"#;