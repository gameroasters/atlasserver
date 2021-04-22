fn main() {
	protobuf_codegen_pure::Codegen::new()
		.out_dir("src/schema")
		.include("proto")
		.input("proto/schema.proto")
		.run()
		.unwrap();

	let schema = std::fs::read("proto/schema.proto")
		.expect("Couldn't read proto/schema.proto");

	let out_dir = std::env::var_os("OUT_DIR").unwrap();
	let dest_path =
		std::path::Path::new(&out_dir).join("get_schema.rs");
	std::fs::write(
		&dest_path,
		format!(
			r#"
        pub fn get_schema_string() -> String {{
            let vec = vec!{:?};
            String::from_utf8(vec).unwrap()
        }}
        "#,
			schema
		),
	)
	.unwrap();
	println!("cargo:rerun-if-changed=proto/schema.proto");
	println!("cargo:rerun-if-changed=build.rs");
}
