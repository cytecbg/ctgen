let input = params[0];
let type = type_of(input);

let result = "";

if type == "string" {
    result = input;
} else if type == "map" {
    result = input.keys()[0];
}

result