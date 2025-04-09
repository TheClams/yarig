// generator_macro/src/lib.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields, LitStr};

#[proc_macro_attribute]
pub fn add_gen_core(attr: TokenStream, item: TokenStream) -> TokenStream {

    // Parse the input tokens into a syntax tree
    let ext = parse_macro_input!(attr as LitStr);
    let input = parse_macro_input!(item as DeriveInput);
    
    // Get the name of the struct
    let name = &input.ident;
    
    // Get the generics to preserve them in the output
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    
    // Define the GeneratorBase impl that will be added
    let ext = ext.value();
    let ext_value = ext.as_str();
    let generator_base_impl = quote! {
        impl #impl_generics GeneratorBase for #name #ty_generics #where_clause {
            const EXT: &'static str = #ext_value;
            fn core(&self) -> &GeneratorCore {
                &self.core
            }
            fn core_mut(&mut self) -> &mut GeneratorCore {
                &mut self.core
            }
        }
    };
    
    // Process based on the type of item (struct, enum, etc.)
    match &input.data {
        Data::Struct(data_struct) => {
            // Handle different field types (named vs unnamed)
            match &data_struct.fields {
                Fields::Named(fields) => {
                    // Get the original fields
                    let fields = &fields.named;
                    
                    // Create the new struct with the additional field and GeneratorBase impl
                    let expanded = quote! {
                        pub struct #name #impl_generics #where_clause {
                            pub core: GeneratorCore,
                            #fields
                        }
                        
                        #generator_base_impl
                    };
                    TokenStream::from(expanded)
                },
                // Handle unit structs
                Fields::Unit => {
                    let expanded = quote! {
                        pub struct #name #impl_generics #where_clause {
                            pub core: GeneratorCore
                        }
                        
                        #generator_base_impl
                    };
                    TokenStream::from(expanded)
                },
                // Return an error for tuple structs
                Fields::Unnamed(_) => {
                    let error = quote! {
                        compile_error!("add_gen_core can only be used on structs");
                    };
                    TokenStream::from(error)
                },
            }
        },
        _ => {
            // Return an error if not a struct
            let error = quote! {
                compile_error!("add_gen_core can only be used on structs");
            };
            TokenStream::from(error)
        }
    }
}
