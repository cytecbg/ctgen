# About
Code generation tool meant to reduce repetitive tasks in day-to-day operations. 

Generate code or text documents based on pre-defined code templates and a database schema.

Currently supports only MySQL/MariaDB databases with InnoDB table storage engine.

Code templates are written in `handlebars` format and support `rhai` scripts.

# Install

Run `cargo install ctgen` (if this project reached the public crate stage).

Or alternatively clone the repository and run `cargo build --release` and then  
copy `target/release/ctgen` to a bin path of your choice.

# Usage

There are 3 modes of operation (commands).
1. The `init` command is for creating a new configuration profile project.
2. The `config` command is for managing existing configuration profiles.
3. The `run` command is for running a generation task inside another project.

## Create profile

To create your first configuration profile go somewhere in your filesystem and run `ctgen init`.  

Optionally you can create the profile project in a new directory by running `ctgen init <dirname>`.

To avoid being prompted for a profile name, use the `--name` option: `ctgen init --name backend backend_templates`.

This will create a new configuration profile project and register it using the same name.

The default project layout is:
- Profile config file: `Ctgen.toml`. Describes the profile behavior, templates and build targets.
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

# Notes

- If a rhai script file is named `op.rhai` inside `assets/scripts`, then you will have `{{op}}` helper available in your handlebars templates
- If your template file is named `backend.hbs` inside `assets/templates`, to define a target that uses that template, use the name `backend` as template name
- Available helpers (other than handlebars' defaults) are: `{{inflect}}` [handlebars-inflector](https://crates.io/crates/handlebars-inflector), `{{concat}}` [handlebars-concat](https://crates.io/crates/handlebars-concat) and `{{json}}` (takes the first argument and turns it into a JSON)
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
  }
}
```

To dump your own context for debugging purposes use `{{{json this}}}` in your template.

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