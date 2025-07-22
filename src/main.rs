mod syntax;


fn main() -> Result<(), &'static str> {
    syntax::parse_file()
}
