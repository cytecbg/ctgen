let input = params[0];
let type = type_of(input);

let result = "";

if type == "string" {
    result = input;
} else if type == "map" {
    let main_part = input.keys()[0];
    let inner_part = "";
    let extra = " ";

    if type_of(input.values()[0]) == "array" {
        for token in input.values()[0] {
            if type_of(token) == "string" && main_part != "enum" && main_part != "set" {
                extra += token;
            } else {
                inner_part += token.to_string() + ","
            }
        }
    } else {
        inner_part = input.values()[0].to_string();
    }

    inner_part = inner_part.sub_string(0, (inner_part.len()-1));
    extra.make_lower();

    result = main_part + "(" + inner_part + ")" + extra;
}

result