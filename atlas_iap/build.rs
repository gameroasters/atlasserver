use std::{
	fs::File,
	io::{self, BufRead, Write},
};

fn main() {
	let out_dir = std::env::var_os("OUT_DIR").unwrap();

	let schema = File::open("proto/schema.proto")
		.expect("Couldn't open proto/schema.proto");
	let mut lines = io::BufReader::new(schema).lines();

	let mut schema_file = File::create(
		std::path::Path::new(&out_dir).join("schema.proto"),
	)
	.expect("failed to create schema.proto output file");

	//TODO: abstract this header insertion
	schema_file
		.write_all(
			(lines
				.next()
				.expect("failed to get first line from schema")
				.expect("first line failed to be read")
				+ "\n")
				.as_bytes(),
		)
		.expect("failed to write line to schema file");

	#[cfg(feature = "json-proto")]
	{
		let header = std::fs::read("proto/header.proto")
			.expect("failed to read header");
		schema_file
			.write_all(&header)
			.expect("failed to write header to schema file");
	}

	lines.for_each(|line| {
		schema_file
			.write_all(
				(line.expect("failed to read line") + "\n")
					.as_bytes(),
			)
			.expect("failed to write line to schema file");
	});

	protobuf_codegen_pure::Codegen::new()
		.out_dir("src/schema")
		.include(std::path::Path::new(&out_dir))
		.input(std::path::Path::new(&out_dir).join("schema.proto"))
		.run()
		.unwrap();

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
			std::fs::read("proto/schema.proto").expect(
				"Couldn't read schema.proto for get_schema_string"
			)
		),
	)
	.unwrap();
	println!("cargo:rerun-if-changed=proto/schema.proto");
	println!("cargo:rerun-if-changed=build.rs");
}
