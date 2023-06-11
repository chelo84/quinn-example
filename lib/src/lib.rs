use proc_macro::TokenStream;
use quote::quote;
use syn::Data::Struct;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

fn generate_read_from_recv_code(fields: &Fields) -> proc_macro2::TokenStream {

    let field_code = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        quote! {
            #field_name: <#field_type as example_core::Payload>::read_from_recv_stream(recv).await?,
        }
    });

    quote! {
        #(#field_code)*
    }
}

fn generate_write_to_send_code(fields: &Fields) -> proc_macro2::TokenStream {
    let field_code = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();

        quote! {
            self.#field_name.write_to_send_stream(send).await?;
        }
    });

    quote! {
        #(#field_code)*
    }
}

#[proc_macro_derive(Payload)]
pub fn derive_payload(input: TokenStream) -> TokenStream {
    let DeriveInput {
        ident,
        data,
        ..
    } = parse_macro_input!(input as DeriveInput);

    let read_from_recv_code = match &data {
        Struct(data) => generate_read_from_recv_code(&data.fields),
        Data::Enum(_) => quote! { compile_error!("enum") },
        Data::Union(_) => quote! { compile_error!("union") }
    };

    let write_to_send_code = match &data {
        Struct(data) => generate_write_to_send_code(&data.fields),
        Data::Enum(_) => quote! { compile_error!("enum") },
        Data::Union(_) => quote! { compile_error!("union") }
    };

    let expanded = quote! {
        #[async_trait::async_trait]
        impl example_core::Payload for #ident {
            async fn read_from_recv_stream(recv: &mut quinn::RecvStream) -> anyhow::Result<#ident> {
                Ok(#ident {
                    #read_from_recv_code
                })
            }

            async fn write_to_send_stream(&self, send: &mut quinn::SendStream) -> anyhow::Result<()> {
                #write_to_send_code

                Ok(())
            }
        }
    };

    expanded.into()
}