// SPDX-License-Identifier: MIT OR Apache-2.0
use std::collections::BTreeMap;
use suture_driver::{DriverError, SemanticChange, SutureDriver};

pub struct SqlDriver;

#[derive(Debug, Clone, PartialEq)]
struct ColumnDef {
    name: String,
    col_type: String,
    constraints: Vec<String>,
}

impl ColumnDef {
    fn signature(&self) -> String {
        format!("{} {}", self.name, self.col_type)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Statement {
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
    DropTable {
        name: String,
    },
    AlterTableAddColumn {
        table: String,
        column: ColumnDef,
    },
    AlterTableDropColumn {
        table: String,
        column_name: String,
    },
    AlterTableAlterColumn {
        table: String,
        column: ColumnDef,
    },
    CreateIndex {
        name: String,
        table: String,
        columns: Vec<String>,
    },
    DropIndex {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct SqlSchema {
    tables: BTreeMap<String, TableDef>,
    indexes: BTreeMap<String, IndexDef>,
}

#[derive(Debug, Clone, PartialEq)]
struct TableDef {
    columns: Vec<ColumnDef>,
}

#[derive(Debug, Clone, PartialEq)]
struct IndexDef {
    table: String,
    columns: Vec<String>,
}

fn normalize_ws(s: &str) -> String {
    let trimmed = s.trim();
    let re: Vec<char> = trimmed
        .chars()
        .fold((Vec::new(), false), |(mut acc, in_space), c| {
            if c.is_whitespace() {
                if !in_space {
                    acc.push(' ');
                }
                (acc, true)
            } else {
                acc.push(c);
                (acc, false)
            }
        })
        .0;
    re.into_iter().collect()
}

fn starts_with_ignore_case(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if starts_with_ignore_case(s, prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn parse_column_def(col_text: &str) -> Option<ColumnDef> {
    let col_text = col_text.trim();
    if col_text.is_empty() {
        return None;
    }
    if col_text.starts_with("--") || col_text.starts_with("/*") {
        return None;
    }

    let parts: Vec<&str> = col_text.splitn(2, char::is_whitespace).collect();
    if parts.len() < 2 {
        return None;
    }

    let name = parts[0].trim().to_owned();
    let rest = parts[1].trim();

    let mut col_type_parts = Vec::new();
    let mut constraints = Vec::new();
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let mut i = 0;

    while i < tokens.len() {
        let tok = tokens[i];
        if tok.eq_ignore_ascii_case("NOT")
            && i + 1 < tokens.len()
            && tokens[i + 1].eq_ignore_ascii_case("NULL")
        {
            constraints.push("NOT NULL".to_owned());
            i += 2;
            continue;
        }
        if tok.eq_ignore_ascii_case("DEFAULT") {
            let mut def_val = String::new();
            i += 1;
            while i < tokens.len() {
                let t = tokens[i];
                if t == "," {
                    break;
                }
                if !def_val.is_empty() {
                    def_val.push(' ');
                }
                def_val.push_str(t);
                i += 1;
            }
            constraints.push(format!("DEFAULT {def_val}"));
            continue;
        }
        if tok.eq_ignore_ascii_case("PRIMARY")
            && i + 1 < tokens.len()
            && tokens[i + 1].eq_ignore_ascii_case("KEY")
        {
            constraints.push("PRIMARY KEY".to_owned());
            i += 2;
            continue;
        }
        if tok.eq_ignore_ascii_case("UNIQUE") {
            constraints.push("UNIQUE".to_owned());
            i += 1;
            continue;
        }
        if tok.eq_ignore_ascii_case("REFERENCES") {
            let mut ref_str = String::new();
            i += 1;
            while i < tokens.len() {
                let t = tokens[i];
                if t == "," {
                    break;
                }
                if !ref_str.is_empty() {
                    ref_str.push(' ');
                }
                ref_str.push_str(t);
                i += 1;
            }
            constraints.push(format!("REFERENCES {ref_str}"));
            continue;
        }
        if tok.eq_ignore_ascii_case("AUTO_INCREMENT") || tok.eq_ignore_ascii_case("AUTOINCREMENT") {
            constraints.push("AUTO_INCREMENT".to_owned());
            i += 1;
            continue;
        }
        if tok.eq_ignore_ascii_case("CHECK") {
            let mut check_str = String::new();
            i += 1;
            let mut depth = 0i32;
            while i < tokens.len() {
                let t = tokens[i];
                if t == "(" {
                    depth += 1;
                } else if t == ")" {
                    depth -= 1;
                    if depth == 0 {
                        check_str.push_str(t);
                        i += 1;
                        break;
                    }
                }
                if !check_str.is_empty() {
                    check_str.push(' ');
                }
                check_str.push_str(t);
                i += 1;
            }
            constraints.push(format!("CHECK {check_str}"));
            continue;
        }
        if tok.eq_ignore_ascii_case("NULL") {
            i += 1;
            continue;
        }
        if tok.starts_with("--") {
            break;
        }
        if tok == "," {
            i += 1;
            continue;
        }
        col_type_parts.push(tok);
        i += 1;
    }

    if col_type_parts.is_empty() {
        return None;
    }

    let col_type = col_type_parts.join(" ");
    Some(ColumnDef {
        name,
        col_type,
        constraints,
    })
}

fn extract_identifier(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.starts_with('"')
        && let Some(end) = s[1..].find('"')
    {
        return Some(&s[1..=end]);
    }
    if s.starts_with('`')
        && let Some(end) = s[1..].find('`')
    {
        return Some(&s[1..=end]);
    }
    if s.starts_with('[')
        && let Some(end) = s[1..].find(']')
    {
        return Some(&s[1..=end]);
    }
    let end = s
        .find(|c: char| c.is_whitespace() || c == '(' || c == ';')
        .unwrap_or(s.len());
    if end == 0 { None } else { Some(&s[..end]) }
}

fn parse_column_list(body: &str) -> Vec<ColumnDef> {
    let body = body.trim();
    let body = body.trim_end_matches(')');
    let mut columns = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();

    for ch in body.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                if let Some(col) = parse_column_def(&current) {
                    columns.push(col);
                }
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if let Some(col) = parse_column_def(&current) {
        columns.push(col);
    }

    columns
}

fn find_on_upper(s: &str) -> Option<usize> {
    let upper = s.to_uppercase();
    upper.find(" ON ")
}

fn parse_statements(content: &str) -> Vec<Statement> {
    let mut statements = Vec::new();
    let raw_stmts: Vec<&str> = content.split(';').collect();

    for raw in raw_stmts {
        let stmt = normalize_ws(raw);

        if stmt.is_empty() {
            continue;
        }

        if let Some(rest) = strip_prefix_ignore_case(&stmt, "CREATE TABLE") {
            let rest = rest.trim();
            let name = extract_identifier(rest);
            if let Some(name) = name
                && let Some(paren_start) = rest.find('(')
            {
                let body = &rest[paren_start + 1..];
                let columns = parse_column_list(body);
                statements.push(Statement::CreateTable {
                    name: name.to_owned(),
                    columns,
                });
                continue;
            }
        }

        if let Some(rest) = strip_prefix_ignore_case(&stmt, "DROP TABLE") {
            let rest = rest.trim();
            let name = extract_identifier(rest);
            if let Some(name) = name {
                statements.push(Statement::DropTable {
                    name: name.to_owned(),
                });
                continue;
            }
        }

        if let Some(rest) = strip_prefix_ignore_case(&stmt, "ALTER TABLE") {
            let rest = rest.trim();
            let table = extract_identifier(rest);
            if let Some(table) = table {
                let after_table = rest[table.len()..].trim();

                if let Some(col_rest) = strip_prefix_ignore_case(after_table, "ADD COLUMN") {
                    let col_rest = col_rest.trim();
                    if let Some(col) = parse_column_def(col_rest) {
                        statements.push(Statement::AlterTableAddColumn {
                            table: table.to_owned(),
                            column: col,
                        });
                        continue;
                    }
                }

                if let Some(col_rest) = strip_prefix_ignore_case(after_table, "ADD") {
                    let col_rest = col_rest.trim();
                    if let Some(col) = parse_column_def(col_rest) {
                        statements.push(Statement::AlterTableAddColumn {
                            table: table.to_owned(),
                            column: col,
                        });
                        continue;
                    }
                }

                if let Some(col_rest) = strip_prefix_ignore_case(after_table, "DROP COLUMN") {
                    let col_rest = col_rest.trim();
                    let col_name = extract_identifier(col_rest);
                    statements.push(Statement::AlterTableDropColumn {
                        table: table.to_owned(),
                        column_name: col_name.unwrap_or(col_rest).to_owned(),
                    });
                    continue;
                }

                if let Some(col_rest) = strip_prefix_ignore_case(after_table, "ALTER COLUMN") {
                    let col_rest = col_rest.trim();
                    if let Some(col) = parse_column_def(col_rest) {
                        statements.push(Statement::AlterTableAlterColumn {
                            table: table.to_owned(),
                            column: col,
                        });
                        continue;
                    }
                }
            }
        }

        if let Some(rest) = strip_prefix_ignore_case(&stmt, "CREATE INDEX") {
            let rest = rest.trim();
            if let Some(on_pos) = find_on_upper(rest) {
                let idx_name = rest[..on_pos].trim().to_owned();
                let after_on = &rest[on_pos + 4..];
                let after_on = after_on.trim();
                let table = extract_identifier(after_on);
                if let Some(table) = table {
                    let after_table = after_on[table.len()..].trim();
                    let after_table = after_table.trim_start_matches('(').trim_end_matches(')');
                    let cols: Vec<String> = after_table
                        .split(',')
                        .map(|c| c.trim().to_owned())
                        .filter(|c| !c.is_empty())
                        .collect();
                    statements.push(Statement::CreateIndex {
                        name: idx_name,
                        table: table.to_owned(),
                        columns: cols,
                    });
                    continue;
                }
            }
        }

        if let Some(rest) = strip_prefix_ignore_case(&stmt, "DROP INDEX") {
            let rest = rest.trim();
            let name = extract_identifier(rest);
            statements.push(Statement::DropIndex {
                name: name.map_or_else(|| rest.to_owned(), std::string::ToString::to_string),
            });
            continue;
        }
    }

    statements
}

fn statements_to_schema(stmts: &[Statement]) -> SqlSchema {
    let mut tables: BTreeMap<String, TableDef> = BTreeMap::new();
    let mut indexes: BTreeMap<String, IndexDef> = BTreeMap::new();

    for stmt in stmts {
        match stmt {
            Statement::CreateTable { name, columns } => {
                tables.insert(
                    name.clone(),
                    TableDef {
                        columns: columns.clone(),
                    },
                );
            }
            Statement::DropTable { name } => {
                tables.remove(name);
            }
            Statement::AlterTableAddColumn { table, column } => {
                if let Some(t) = tables.get_mut(table)
                    && !t.columns.iter().any(|c| c.name == column.name)
                {
                    t.columns.push(column.clone());
                }
            }
            Statement::AlterTableDropColumn { table, column_name } => {
                if let Some(t) = tables.get_mut(table) {
                    t.columns.retain(|c| c.name != *column_name);
                }
            }
            Statement::AlterTableAlterColumn { table, column } => {
                if let Some(t) = tables.get_mut(table)
                    && let Some(existing) = t.columns.iter_mut().find(|c| c.name == column.name)
                {
                    *existing = column.clone();
                }
            }
            Statement::CreateIndex {
                name,
                table,
                columns,
            } => {
                indexes.insert(
                    name.clone(),
                    IndexDef {
                        table: table.clone(),
                        columns: columns.clone(),
                    },
                );
            }
            Statement::DropIndex { name } => {
                indexes.remove(name);
            }
        }
    }

    SqlSchema { tables, indexes }
}

fn parse_schema(content: &str) -> SqlSchema {
    let stmts = parse_statements(content);
    statements_to_schema(&stmts)
}

impl SqlDriver {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn diff_schemas(old: &SqlSchema, new: &SqlSchema) -> Vec<SemanticChange> {
        let mut changes = Vec::new();

        let old_tables: std::collections::HashSet<&str> =
            old.tables.keys().map(std::string::String::as_str).collect();
        let new_tables: std::collections::HashSet<&str> =
            new.tables.keys().map(std::string::String::as_str).collect();

        for name in &old_tables {
            if !new_tables.contains(name) {
                let cols_str = old.tables[*name]
                    .columns
                    .iter()
                    .map(ColumnDef::signature)
                    .collect::<Vec<_>>()
                    .join(", ");
                changes.push(SemanticChange::Removed {
                    path: format!("/tables/{name}"),
                    old_value: format!("CREATE TABLE {name} ({cols_str})"),
                });
            }
        }

        for name in &new_tables {
            if !old_tables.contains(name) {
                let cols_str = new.tables[*name]
                    .columns
                    .iter()
                    .map(ColumnDef::signature)
                    .collect::<Vec<_>>()
                    .join(", ");
                changes.push(SemanticChange::Added {
                    path: format!("/tables/{name}"),
                    value: format!("CREATE TABLE {name} ({cols_str})"),
                });
            }
        }

        for name in &old_tables {
            if new_tables.contains(name) {
                let old_tbl = &old.tables[*name];
                let new_tbl = &new.tables[*name];

                let old_cols: std::collections::HashMap<&str, &ColumnDef> = old_tbl
                    .columns
                    .iter()
                    .map(|c| (c.name.as_str(), c))
                    .collect();
                let new_cols: std::collections::HashMap<&str, &ColumnDef> = new_tbl
                    .columns
                    .iter()
                    .map(|c| (c.name.as_str(), c))
                    .collect();

                let old_col_names: std::collections::HashSet<&str> =
                    old_cols.keys().copied().collect();
                let new_col_names: std::collections::HashSet<&str> =
                    new_cols.keys().copied().collect();

                for col_name in &old_col_names {
                    if !new_col_names.contains(col_name) {
                        changes.push(SemanticChange::Removed {
                            path: format!("/tables/{name}/columns/{col_name}"),
                            old_value: old_cols[*col_name].signature(),
                        });
                    }
                }

                for col_name in &new_col_names {
                    if !old_col_names.contains(col_name) {
                        changes.push(SemanticChange::Added {
                            path: format!("/tables/{name}/columns/{col_name}"),
                            value: new_cols[*col_name].signature(),
                        });
                    }
                }

                for col_name in &old_col_names {
                    if let Some(new_col) = new_col_names
                        .contains(col_name)
                        .then(|| new_cols[*col_name])
                    {
                        let old_col = old_cols[*col_name];
                        if old_col != new_col {
                            if old_col.col_type != new_col.col_type {
                                changes.push(SemanticChange::Modified {
                                    path: format!("/tables/{name}/columns/{col_name}/type"),
                                    old_value: old_col.col_type.clone(),
                                    new_value: new_col.col_type.clone(),
                                });
                            }
                            if old_col.constraints != new_col.constraints {
                                changes.push(SemanticChange::Modified {
                                    path: format!("/tables/{name}/columns/{col_name}/constraints"),
                                    old_value: old_col.constraints.join(", "),
                                    new_value: new_col.constraints.join(", "),
                                });
                            }
                        }
                    }
                }
            }
        }

        let old_idx: std::collections::HashSet<&str> = old
            .indexes
            .keys()
            .map(std::string::String::as_str)
            .collect();
        let new_idx: std::collections::HashSet<&str> = new
            .indexes
            .keys()
            .map(std::string::String::as_str)
            .collect();

        for name in &old_idx {
            if !new_idx.contains(name) {
                changes.push(SemanticChange::Removed {
                    path: format!("/indexes/{name}"),
                    old_value: format!(
                        "CREATE INDEX {} ON {} ({})",
                        name,
                        old.indexes[*name].table,
                        old.indexes[*name].columns.join(", ")
                    ),
                });
            }
        }

        for name in &new_idx {
            if !old_idx.contains(name) {
                changes.push(SemanticChange::Added {
                    path: format!("/indexes/{name}"),
                    value: format!(
                        "CREATE INDEX {} ON {} ({})",
                        name,
                        new.indexes[*name].table,
                        new.indexes[*name].columns.join(", ")
                    ),
                });
            }
        }

        for name in &old_idx {
            if new_idx.contains(name) {
                let old_i = &old.indexes[*name];
                let new_i = &new.indexes[*name];
                if old_i != new_i {
                    changes.push(SemanticChange::Modified {
                        path: format!("/indexes/{name}"),
                        old_value: format!(
                            "CREATE INDEX {} ON {} ({})",
                            name,
                            old_i.table,
                            old_i.columns.join(", ")
                        ),
                        new_value: format!(
                            "CREATE INDEX {} ON {} ({})",
                            name,
                            new_i.table,
                            new_i.columns.join(", ")
                        ),
                    });
                }
            }
        }

        changes
    }

    fn schema_to_sql(schema: &SqlSchema) -> String {
        let mut lines: Vec<String> = Vec::new();

        for (name, table) in &schema.tables {
            let col_strs: Vec<String> = table
                .columns
                .iter()
                .map(|c| {
                    let mut parts = vec![c.name.clone(), c.col_type.clone()];
                    parts.extend(c.constraints.clone());
                    parts.join(" ")
                })
                .collect();
            lines.push(format!(
                "CREATE TABLE {name} ({})",
                col_strs.join(",\n    ")
            ));
        }

        for (name, idx) in &schema.indexes {
            lines.push(format!(
                "CREATE INDEX {name} ON {} ({})",
                idx.table,
                idx.columns.join(", ")
            ));
        }

        lines.join(";\n\n") + ";\n"
    }

    fn merge_schemas(base: &SqlSchema, ours: &SqlSchema, theirs: &SqlSchema) -> Option<SqlSchema> {
        let mut tables: BTreeMap<String, Option<TableDef>> = BTreeMap::new();

        let all_table_names: std::collections::HashSet<&str> = base
            .tables
            .keys()
            .chain(ours.tables.keys())
            .chain(theirs.tables.keys())
            .map(std::string::String::as_str)
            .collect();

        for name in &all_table_names {
            let in_base = base.tables.get(*name);
            let in_ours = ours.tables.get(*name);
            let in_theirs = theirs.tables.get(*name);

            match (in_base, in_ours, in_theirs) {
                (None | Some(_), None, None) => {}
                (None | Some(_), Some(o), None) => {
                    tables.insert(name.to_string(), Some(o.clone()));
                }
                (None | Some(_), None, Some(t)) => {
                    tables.insert(name.to_string(), Some(t.clone()));
                }
                (None, Some(o), Some(t)) => {
                    if o == t {
                        tables.insert(name.to_string(), Some(o.clone()));
                    } else {
                        return None;
                    }
                }
                (Some(b), Some(o), Some(t)) => {
                    if o == t {
                        tables.insert(name.to_string(), Some(o.clone()));
                    } else if o == b {
                        tables.insert(name.to_string(), Some(t.clone()));
                    } else if t == b {
                        tables.insert(name.to_string(), Some(o.clone()));
                    } else {
                        let merged_cols = merge_columns(&b.columns, &o.columns, &t.columns);
                        match merged_cols {
                            Some(cols) => {
                                tables.insert(name.to_string(), Some(TableDef { columns: cols }));
                            }
                            None => return None,
                        }
                    }
                }
            }
        }

        let mut merged_indexes: BTreeMap<String, IndexDef> = BTreeMap::new();

        let all_idx_names: std::collections::HashSet<&str> = base
            .indexes
            .keys()
            .chain(ours.indexes.keys())
            .chain(theirs.indexes.keys())
            .map(std::string::String::as_str)
            .collect();

        for name in &all_idx_names {
            let in_base = base.indexes.get(*name);
            let in_ours = ours.indexes.get(*name);
            let in_theirs = theirs.indexes.get(*name);

            match (in_base, in_ours, in_theirs) {
                (None | Some(_), None, None) => {}
                (None | Some(_), Some(o), None) => {
                    merged_indexes.insert(name.to_string(), o.clone());
                }
                (None | Some(_), None, Some(t)) => {
                    merged_indexes.insert(name.to_string(), t.clone());
                }
                (None | Some(_), Some(o), Some(t)) => {
                    if o == t {
                        merged_indexes.insert(name.to_string(), o.clone());
                    } else {
                        return None;
                    }
                }
            }
        }

        let mut final_tables = BTreeMap::new();
        for (name, tbl) in tables {
            if let Some(t) = tbl {
                final_tables.insert(name, t);
            }
        }

        Some(SqlSchema {
            tables: final_tables,
            indexes: merged_indexes,
        })
    }

    fn format_change(change: &SemanticChange) -> String {
        match change {
            SemanticChange::Added { path, value } => {
                format!("  ADDED     {path}: {value}")
            }
            SemanticChange::Removed { path, old_value } => {
                format!("  REMOVED   {path}: {old_value}")
            }
            SemanticChange::Modified {
                path,
                old_value,
                new_value,
            } => {
                format!("  MODIFIED  {path}: {old_value} -> {new_value}")
            }
            SemanticChange::Moved {
                old_path,
                new_path,
                value,
            } => {
                format!("  MOVED     {old_path} -> {new_path}: {value}")
            }
        }
    }
}

fn merge_columns(
    base: &[ColumnDef],
    ours: &[ColumnDef],
    theirs: &[ColumnDef],
) -> Option<Vec<ColumnDef>> {
    let base_map: std::collections::HashMap<&str, &ColumnDef> =
        base.iter().map(|c| (c.name.as_str(), c)).collect();
    let ours_map: std::collections::HashMap<&str, &ColumnDef> =
        ours.iter().map(|c| (c.name.as_str(), c)).collect();
    let theirs_map: std::collections::HashMap<&str, &ColumnDef> =
        theirs.iter().map(|c| (c.name.as_str(), c)).collect();

    let all_names: std::collections::HashSet<&str> = base_map
        .keys()
        .chain(ours_map.keys())
        .chain(theirs_map.keys())
        .copied()
        .collect();

    let mut merged = Vec::new();

    for name in &all_names {
        let in_base = base_map.get(name);
        let in_ours = ours_map.get(name);
        let in_theirs = theirs_map.get(name);

        match (in_base, in_ours, in_theirs) {
            (None | Some(_), None, None) => {}
            (None | Some(_), Some(o), None) => merged.push((*o).clone()),
            (None | Some(_), None, Some(t)) => merged.push((*t).clone()),
            (None, Some(o), Some(t)) => {
                if o == t {
                    merged.push((*o).clone());
                } else {
                    return None;
                }
            }
            (Some(b), Some(o), Some(t)) => {
                if o == t {
                    merged.push((*o).clone());
                } else if o == b {
                    merged.push((*t).clone());
                } else if t == b {
                    merged.push((*o).clone());
                } else {
                    return None;
                }
            }
        }
    }

    merged.sort_by(|a, b| a.name.cmp(&b.name));

    Some(merged)
}

impl Default for SqlDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SutureDriver for SqlDriver {
    fn name(&self) -> &'static str {
        "SQL"
    }

    fn supported_extensions(&self) -> &[&str] {
        &[".sql"]
    }

    fn diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<Vec<SemanticChange>, DriverError> {
        let new_schema = parse_schema(new_content);

        match base_content {
            None => {
                let mut changes = Vec::new();
                for (name, table) in &new_schema.tables {
                    let cols_str = table
                        .columns
                        .iter()
                        .map(ColumnDef::signature)
                        .collect::<Vec<_>>()
                        .join(", ");
                    changes.push(SemanticChange::Added {
                        path: format!("/tables/{name}"),
                        value: format!("CREATE TABLE {name} ({cols_str})"),
                    });
                }
                for (name, idx) in &new_schema.indexes {
                    changes.push(SemanticChange::Added {
                        path: format!("/indexes/{name}"),
                        value: format!(
                            "CREATE INDEX {} ON {} ({})",
                            name,
                            idx.table,
                            idx.columns.join(", ")
                        ),
                    });
                }
                Ok(changes)
            }
            Some(base) => {
                let old_schema = parse_schema(base);
                Ok(Self::diff_schemas(&old_schema, &new_schema))
            }
        }
    }

    fn format_diff(
        &self,
        base_content: Option<&str>,
        new_content: &str,
    ) -> Result<String, DriverError> {
        let changes = self.diff(base_content, new_content)?;

        if changes.is_empty() {
            return Ok("no changes".to_owned());
        }

        let lines: Vec<String> = changes.iter().map(Self::format_change).collect();
        Ok(lines.join("\n"))
    }

    fn merge(&self, base: &str, ours: &str, theirs: &str) -> Result<Option<String>, DriverError> {
        let base_schema = parse_schema(base);
        let ours_schema = parse_schema(ours);
        let theirs_schema = parse_schema(theirs);

        Ok(
            Self::merge_schemas(&base_schema, &ours_schema, &theirs_schema)
                .map(|merged| Self::schema_to_sql(&merged)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prop_assert;
    use proptest::prop_assume;
    use proptest::proptest;

    #[test]
    fn test_sql_driver_name() {
        let driver = SqlDriver::new();
        assert_eq!(driver.name(), "SQL");
    }

    #[test]
    fn test_sql_driver_extensions() {
        let driver = SqlDriver::new();
        assert_eq!(driver.supported_extensions(), &[".sql"]);
    }

    #[test]
    fn test_sql_diff_added_table() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n\nCREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT);\n";

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path == "/tables/posts"
        )));
    }

    #[test]
    fn test_sql_diff_removed_table() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n\nCREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Removed { path, .. } if path == "/tables/posts"
        )));
    }

    #[test]
    fn test_sql_diff_added_column() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);\n";

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path == "/tables/users/columns/email"
        )));
    }

    #[test]
    fn test_sql_diff_removed_column() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Removed { path, .. } if path == "/tables/users/columns/email"
        )));
    }

    #[test]
    fn test_sql_diff_modified_column() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);\n";

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, value, .. }
                if path == "/tables/users/columns/email" && value == "email TEXT"
        )));
    }

    #[test]
    fn test_sql_diff_type_change() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, age INTEGER);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, age TEXT);\n";

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified { path, old_value, new_value, .. }
                if path == "/tables/users/columns/age/type"
                    && old_value == "INTEGER"
                    && new_value == "TEXT"
        )));
    }

    #[test]
    fn test_sql_diff_constraint_change() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER, name TEXT);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Modified { path, .. }
                if path == "/tables/users/columns/id/constraints"
        )));
    }

    #[test]
    fn test_sql_diff_new_file() {
        let driver = SqlDriver::new();
        let content = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";

        let changes = driver.diff(None, content).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(
            &changes[0],
            SemanticChange::Added { path, .. } if path == "/tables/users"
        ));
    }

    #[test]
    fn test_sql_merge_no_conflict() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let ours = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);\n";
        let theirs =
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER);\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        assert!(merged.contains("email"));
        assert!(merged.contains("age"));
    }

    #[test]
    fn test_sql_merge_conflict() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let ours =
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER);\n";
        let theirs = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age TEXT);\n";

        let result = driver.merge(base, ours, theirs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_sql_diff_empty() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_sql_format_diff() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n\nCREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT);\n";

        let formatted = driver.format_diff(Some(base), new).unwrap();
        assert!(formatted.contains("ADDED"));
        assert!(formatted.contains("/tables/posts"));
    }

    #[test]
    fn test_sql_diff_index_changes() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n\nCREATE INDEX idx_users_name ON users (name);\n";

        let changes = driver.diff(Some(base), new).unwrap();
        assert!(changes.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path == "/indexes/idx_users_name"
        )));
    }

    #[test]
    fn test_sql_diff_multiple_tables() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let new = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);\n\nCREATE TABLE posts (id INTEGER PRIMARY KEY, title TEXT);\n\nCREATE TABLE comments (id INTEGER PRIMARY KEY, body TEXT);\n";

        let changes = driver.diff(Some(base), new).unwrap();
        let added_tables: Vec<_> = changes
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    SemanticChange::Added { path, .. } if path.starts_with("/tables/")
                )
            })
            .collect();
        assert!(added_tables.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path == "/tables/posts"
        )));
        assert!(added_tables.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path == "/tables/comments"
        )));
        assert!(added_tables.iter().any(|c| matches!(
            c,
            SemanticChange::Added { path, .. } if path == "/tables/users/columns/email"
        )));
    }

    #[test]
    fn test_correctness_merge_determinism() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let ours = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);\n";
        let theirs =
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER);\n";

        let r1 = driver.merge(base, ours, theirs).unwrap();
        let r2 = driver.merge(base, theirs, ours).unwrap();
        assert_eq!(r1.is_some(), r2.is_some());
        if let (Some(m1), Some(m2)) = (r1, r2) {
            let s1 = parse_schema(&m1);
            let s2 = parse_schema(&m2);
            assert_eq!(s1, s2, "merge must be commutative");
        }
    }

    #[test]
    fn test_correctness_merge_idempotency() {
        let driver = SqlDriver::new();
        let base = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);\n";
        let ours = "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);\n";

        let result = driver.merge(base, ours, ours).unwrap();
        assert!(result.is_some());
        let merged = result.unwrap();
        let merged_schema = parse_schema(&merged);
        let ours_schema = parse_schema(ours);
        assert_eq!(
            merged_schema, ours_schema,
            "merge(base, ours, ours) should equal ours"
        );
    }

    proptest! {
        #[test]
        fn test_merge_identity(table in "[a-z][a-z0-9]*", col in "[a-z][a-z0-9]*") {
            let sql = format!("CREATE TABLE {} ({} INTEGER);", table, col);
            let driver = SqlDriver::new();
            let result = driver.merge(&sql, &sql, &sql).unwrap();
            prop_assert!(result.is_some());
        }

        #[test]
        fn test_merge_idempotence(
            table in "[a-z][a-z0-9]*",
            col1 in "[a-z][a-z0-9]*",
            col2 in "[a-z][a-z0-9]*",
        ) {
            let base = format!("CREATE TABLE {} ({} INTEGER);", table, col1);
            let modified = format!("CREATE TABLE {} ({} INTEGER, {} TEXT);", table, col1, col2);
            let driver = SqlDriver::new();
            let result = driver.merge(&base, &modified, &modified).unwrap();
            prop_assert!(result.is_some());
            let merged = result.unwrap();
            prop_assert!(merged.contains(&col2), "added column should be present");
        }

        #[test]
        fn test_sql_merge_non_overlapping_ddl(
            table_name in "[a-z][a-z0-9]*",
            col1 in "[a-z][a-z0-9]*",
            col2 in "[a-z][a-z0-9]*",
            col3 in "[a-z][a-z0-9]*",
            default_val in "[a-z0-9]+",
        ) {
            // Skip degenerate case where col1 == col3 (same column added twice)
            prop_assume!(col1 != col3);
            let base = format!("CREATE TABLE {} ({} INTEGER, {} TEXT);", table_name, col1, col2);
            let ours = format!("CREATE TABLE {} ({} INTEGER, {} TEXT, {} REAL);", table_name, col1, col2, col3);
            let theirs = format!("CREATE TABLE {} ({} INTEGER DEFAULT {}, {} TEXT);", table_name, col1, default_val, col2);

            let driver = SqlDriver::new();
            let result = driver.merge(&base, &ours, &theirs);
            prop_assert!(result.is_ok());
            prop_assert!(result.unwrap().is_some());
        }
    }
}
