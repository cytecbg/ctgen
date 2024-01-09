let database = params[0];
let col_count = 0;

for table in database.tables.values() {
    for col in table.columns.values() {
        col_count += 1;
    }
}

col_count