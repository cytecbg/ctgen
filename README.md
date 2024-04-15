[![Crates.io](https://img.shields.io/crates/v/ctgen?color=4d76ae)](https://crates.io/crates/ctgen)
[![API](https://docs.rs/ctgen/badge.svg)](https://docs.rs/ctgen)
[![dependency status](https://deps.rs/repo/github/cytecbg/ctgen/status.svg)](https://deps.rs/repo/github/cytecbg/ctgen)
[![build](https://github.com/cytecbg/ctgen/actions/workflows/rust.yml/badge.svg)](https://github.com/cytecbg/ctgen/actions/workflows/rust.yml)

# About
Code generation tool meant to reduce repetitive tasks in day-to-day operations. 

Generate code or text documents based on pre-defined code templates and a database schema.

Currently supports only MySQL/MariaDB databases with InnoDB table storage engine.

Code templates are written in [`handlebars`](https://handlebarsjs.com/guide/) format and support [`rhai`](https://rhai.rs/book/) scripts.

# Install

Run `cargo install ctgen` (if this project reached the public crate stage).

Or alternatively clone the repository and run `cargo build --release` and then  
copy `target/release/ctgen` to a bin path of your choice.

Or just `cargo install --path .`

To see some hands-on examples, check [ctgen-samples](https://github.com/cytecbg/ctgen-samples).

# Usage

There are 3 modes of operation (commands).
1. The [`init`](#create-profile) command is for creating a new configuration profile project.
2. The [`config`](#manage-profiles) command is for managing existing configuration profiles.
3. The [`run`](#run-tasks) command is for running a generation task inside another project.

# Disclaimer

Under no circumstances should you ever run generation tasks based on templates you are not very well familiar with! This poses a great security threat!
When using `ctgen` with templates that you did not create yourself you should read and study the code carefully **before** running any tasks with 
that template! Ctgen can and does modify your local filesystem and has the capability to execute additional shell commands with or without user input!

Under no circumstances should you ever run `ctgen` as `root` or any other privileged account! As of time of writing `ctgen` does NOT have any mechanisms to 
predict or prevent any potentially negative outcomes or dangerous operations. Use discretion and study the templates you use **before** attempting to run any tasks!

## Create profile

To create your first configuration profile go somewhere in your filesystem and run `ctgen init`.  

Optionally you can create the profile project in a new directory by running `ctgen init <dirname>`.

To avoid being prompted for a profile name, use the `--name` option: `ctgen init --name backend backend_templates`.

This will create a new configuration profile project and register it using the same name.

The default project layout is:
- Profile config file: [`Ctgen.toml`](#profile-toml-schema). Describes the profile behavior, templates and build targets.
- Templates directory: `assets/templates`. Contains all `handlebars` templates with `.hbs` extension. The main part of the filename is the template name.
- Scripts directory: `assets/scripts`. Contains all `rhai` scripts with `.rhai` extension. The main part of the filename is used to register the script as handlebars helper.

## Manage profiles

- To add an existing configuration profile to the registry, run `ctgen config add [path to Ctgen.toml]`. If you are in the profile directory, you can just run `ctgen config add`. Optionally you can pass `--default` to override your default profile or `--name my_name` to override the profile name. Otherwise the name of the profile is used in the registry.
- To list registered profiles, run `ctgen config ls`. If an item in the list is blinking in red, that means that the profile is broken and the config file does not exist.
- To remove a profile from the registry, run `ctgen config rm profile_name`.

## Run tasks

Assuming you have a valid configuration profile setup already (see above), to run a generation task you need to:

- Go inside your project: `cd my_awesome_project`
- Run `ctgen run`
- Answer prompts
- ???
- PROFIT

Check `ctgen help run` for extra options like: 

- Choosing a profile other than the `default` using `--profile=flutter`
- Overriding the profile setting for `.env` file, environment variable name, DSN string or target path
- Overriding the profile prompts with the `--prompt` option, for example `--prompt "dummy=1"`. Prompts answered with command-line params will be skipped during the run.

Example runs:

Let's imagine you are generating flutter code for your mobile project. Your profile is called `mobile`. It doesn't know where your database is so you enter it manually. You chose to generate code for table `clients` and know that the profile will ask you whether you want to generate a password reset flow and also add login with Google.

Run: `ctgen run --profile=mobile --dsn="mysql://root@127.0.0.1:3306/project_db" --prompt "password_reset=1" --prompt "google_auth=1" clients`

# Profile TOML Schema

The `Ctgen.toml` file describes the profile behavior and follows this set of rules:

1. The first section in the file is called `profile`, this section holds these fields:
- field `name`: the default profile name
- field `env-file`: the name of the env file to look for when trying to initialize context, typically `.env`
- field `env-var`: the name of the env variable to look for in the `.env` file, for example `DATABASE_CONNECTION`; the value of the variable is expected to be a valid DSN
- field `dsn`: if `env-file` and `env-var` are left empty, the profile could have a hardcoded database DSN instead; otherwise this field could be omitted or left blank
- field `target-dir`: this is the directory that should hold all build targets. It is relative to current working dir when running a generation task (`ctgen run`). CWD is used if left blank
- field `templates-dir`: this is the directory that holds all handlebars templates. It is relative to the profile containing directory.
- field `scripts-dir`: this is the directory that holds all rhai scripts. It is relative to the profile containing directory.
- field `prompts`: this is an array of strings. Every string in the array must be a valid prompt ID of a prompt defined in the `prompt` sections that follow.
- field `targets`: this is an array of strings. Every string in the array must be a valid target ID of a target defined in the `target` sections that follow.
2. Any number of `prompt` sections after the `profile` section declare profile prompts by assigning a prompt ID as a dot-nested value to the section name, for example `[prompt.dummy]`. A prompt can have the following fields (properties):
- field `condition`: optional, containing an inline handlebars template that should render `1` to trigger this prompt
- field `prompt`: containing plain text or an inline handlebars template that is being rendered to the user as prompt text
- field `options`: optional, containing either an array or table (object) of available options (for select and multiselect prompts), or string (for input prompts), or an inline handlebars template that renders a comma-separated list of options (for select and multi-select prompts)
- field `multiple`: optional, boolean flag indicating a multi-select; default is `false`
- field `ordered`: optional, boolean flag indicating that order matters for multi-select values; default is `false`
- field `required`: optional, boolean flag indicating that empty values will not be accepted; default is `false`
3. Any number of `target` sections after the `prompt` sections declare profile build targets by assigning a target ID as a dot-nested value to the section name, for example `[target.dummy]`. A target can have the following fields (properties):
- field `condition`: optional, containing an inline handlebars template that should render `1` to trigger this target to be rendered
- field `template`: string containing a template name, which should exist as a file with `.hbs` extension in the `templates-dir` directory. For example `dummy`, or `backend/dummy`.
- field `target`: string containing an inline handlebars template that should render to a file path inside the `target-dir`. Missing path elements will be created. Could also be plain text path like `main.rs`.
- field `formatter`: optional, containing an inline handlebars template that should render a valid shell command to execute after the target has been rendered and written to disk. Could also be plain text shell command if no context conditional parameters are necessary. NOTE: The only available variable to render is `{{target}}`.

# Notes

- If a rhai script file is named `op.rhai` inside `assets/scripts`, then you will have `{{op}}` helper available in your handlebars templates
- If your template file is named `backend.hbs` inside `assets/templates`, to define a target that uses that template, use the name `backend` as template name
- Available helpers (other than [handlebars](https://handlebarsjs.com/guide/builtin-helpers.html#if)' defaults) are: `{{inflect}}` [handlebars-inflector](https://crates.io/crates/handlebars-inflector), `{{concat}}` [handlebars-concat](https://crates.io/crates/handlebars-concat), `{{datetime}}` [handlebars-chrono](https://crates.io/crates/handlebars-chrono)  and `{{json}}` (takes the first argument and turns it into a JSON)
- The context available during rendering handlebars templates looks roughly like:

```json
{
  "database": {
    "name": "db_name",
    "tables": [],
    "constraints": [],
    "metadata": {}
  },
  "table_name": "selected_table_name",
  "table": {
    "name": "selected_table_name",
    "primary_key": [],
    "columns": [],
    "indexes": [],
    "metadata": {}
  },
  "constraints_local": [],
  "constraints_foreign": [],
  "prompts": {
    "dummy": "1"
  },
  "timestamp": "2024-03-18T21:35:09.750752900+00:00",
  "ctgen_ver": "0.1.2"
}
```

To dump your own context for debugging purposes use `{{{json this}}}` in your template.

# Acknowledgements

This tool relies heavily on [handlebars-rust](https://github.com/sunng87/handlebars-rust/) and [rhai](https://github.com/rhaiscript/rhai/) crates. :heart:

# TODO
- improve error handling
- improve logging
- improve `database-reflection` (add more adapters)
- improve `lib` layout and exports and isolate `cli` junk better
- more data sources, different context structs and context builders
- add dump/load context option to repeat runs
- more handlebars helpers
- maybe template updates by target reverse-engineering? based on diff?
- maybe fs watch to re-run task when templates change
- comments and documentation
- init produces crappy prompt options because of toml and serde circle of hell