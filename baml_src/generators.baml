// This helps use auto generate libraries you can use in the language of
// your choice. You can have multiple generators if you use multiple languages.
// Just ensure that the output_dir is different for each generator.

// OpenAPI/REST generator for Rust integration
// Rust services will call the BAML server via REST API (baml serve)
generator openapi_spec {
    output_type "rest/openapi"
    output_dir "../baml_openapi"
    version "0.213.0"
    default_client_mode async
}
