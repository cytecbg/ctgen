let database = params[0];
let idx_count = 0;

for table in database.tables.values() {
    for index in table.indexes.values() {
        if index.name != "PRIMARY" {
            idx_count += 1;
        }
    }
}

idx_count