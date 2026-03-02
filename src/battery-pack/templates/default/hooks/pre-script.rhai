// Ensure the project name ends with -battery-pack
let name = variable::get("project-name");

if !name.ends_with("-battery-pack") {
    let fixed = name + "-battery-pack";
    print("Renaming project to: " + fixed);
    variable::set("project-name", fixed);
}
