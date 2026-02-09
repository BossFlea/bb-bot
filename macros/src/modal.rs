use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Expr, Ident, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

pub fn define_modal(input: TokenStream) -> TokenStream {
    let modal_def = parse_macro_input!(input as ModalDefinition);

    match generate_modal_code(modal_def) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

// AST structs for input parsing
struct ModalDefinition {
    name: Ident,
    custom_id: Expr,
    title: Expr,
    components: Vec<ModalComponent>,
}

#[allow(clippy::large_enum_variant)] // only at compile-time
enum ModalComponent {
    Input(InputComponent),
    Select(SelectComponent),
    Text(TextComponent),
}

struct InputComponent {
    field_name: Ident,
    style: Expr, // InputTextStyle expression
    label: Expr,
    description: Option<Expr>,
    min_length: Option<Expr>,
    max_length: Option<Expr>,
    required: Option<Expr>,
    placeholder: Option<Expr>,
}

struct SelectComponent {
    field_name: Ident,
    kind: Expr, // CreateSelectMenuKind expression
    label: Expr,
    description: Option<Expr>,
    min_values: Option<Expr>,
    max_values: Option<Expr>,
    required: Option<Expr>,
    placeholder: Option<Expr>,
}

enum SelectKindVariant {
    String,
    User,
    Role,
    Mentionable,
    Channel,
    Unknown,
}

struct TextComponent {
    content: Expr,
}

fn extract_select_kind_variant(expr: &Expr) -> SelectKindVariant {
    match expr {
        // match CreateSelectMenuKind::String { ... }
        Expr::Struct(expr_struct) => {
            if let Some(last_segment) = expr_struct.path.segments.last() {
                match last_segment.ident.to_string().as_str() {
                    "String" => SelectKindVariant::String,
                    "User" => SelectKindVariant::User,
                    "Role" => SelectKindVariant::Role,
                    "Mentionable" => SelectKindVariant::Mentionable,
                    "Channel" => SelectKindVariant::Channel,
                    _ => SelectKindVariant::Unknown,
                }
            } else {
                SelectKindVariant::Unknown
            }
        }
        // won't bother with more complex expressions
        _ => SelectKindVariant::Unknown,
    }
}

impl Parse for ModalDefinition {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;

        let content;
        syn::braced!(content in input);

        let mut custom_id = None;
        let mut title = None;
        let mut components = None;

        while !content.is_empty() {
            let field_ident: Ident = content.parse()?;
            content.parse::<Token![:]>()?;

            match field_ident.to_string().as_str() {
                "custom_id" => {
                    if custom_id.is_some() {
                        return Err(syn::Error::new(
                            field_ident.span(),
                            "Duplicate 'custom_id' field",
                        ));
                    }
                    custom_id = Some(content.parse::<Expr>()?);
                }
                "title" => {
                    if title.is_some() {
                        return Err(syn::Error::new(
                            field_ident.span(),
                            "Duplicate 'title' field",
                        ));
                    }
                    title = Some(content.parse::<Expr>()?);
                }
                "components" => {
                    if components.is_some() {
                        return Err(syn::Error::new(
                            field_ident.span(),
                            "Duplicate 'components' field",
                        ));
                    }

                    let components_content;
                    syn::bracketed!(components_content in content);

                    let mut component_list = Vec::new();
                    while !components_content.is_empty() {
                        let component = components_content.parse::<ModalComponent>()?;
                        component_list.push(component);

                        if components_content.peek(Token![,]) {
                            components_content.parse::<Token![,]>()?;
                        }
                    }
                    components = Some(component_list);
                }
                _ => {
                    return Err(syn::Error::new(
                        field_ident.span(),
                        "Unknown field. Expected 'custom_id', 'title', or 'components'",
                    ));
                }
            }

            // handle trailing comma
            if content.peek(Token![,]) {
                content.parse::<Token![,]>()?;
            }
        }

        let custom_id = custom_id
            .ok_or_else(|| syn::Error::new(input.span(), "Missing required field 'custom_id'"))?;
        let title =
            title.ok_or_else(|| syn::Error::new(input.span(), "Missing required field 'title'"))?;
        let components = components
            .ok_or_else(|| syn::Error::new(input.span(), "Missing required field 'components'"))?;

        Ok(ModalDefinition {
            name,
            custom_id,
            title,
            components,
        })
    }
}

impl Parse for ModalComponent {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;
        match ident.to_string().as_str() {
            "input" => {
                let input_comp = parse_input_component(input)?;
                Ok(ModalComponent::Input(input_comp))
            }
            "select" => {
                let select_comp = parse_select_component(input)?;
                Ok(ModalComponent::Select(select_comp))
            }
            "text" => {
                let text_comp = parse_text_component(input)?;
                Ok(ModalComponent::Text(text_comp))
            }
            _ => Err(syn::Error::new(
                ident.span(),
                "Unknown component. Expected 'input', 'select' or 'text'",
            )),
        }
    }
}

fn parse_input_component(input: ParseStream) -> Result<InputComponent> {
    let field_name: Ident = input.parse()?;

    let content;
    syn::braced!(content in input);

    let mut style = None;
    let mut label = None;
    let mut description = None;
    let mut min_length = None;
    let mut max_length = None;
    let mut required = None;
    let mut placeholder = None;

    while !content.is_empty() {
        let field: Ident = content.parse()?;
        content.parse::<Token![:]>()?;

        match field.to_string().as_str() {
            "style" => {
                style = Some(content.parse::<Expr>()?);
            }
            "label" => {
                label = Some(content.parse::<Expr>()?);
            }
            "description" => {
                description = Some(content.parse::<Expr>()?);
            }
            "min_length" => {
                min_length = Some(content.parse::<Expr>()?);
            }
            "max_length" => {
                max_length = Some(content.parse::<Expr>()?);
            }
            "required" => {
                required = Some(content.parse::<Expr>()?);
            }
            "placeholder" => {
                placeholder = Some(content.parse::<Expr>()?);
            }
            _ => return Err(syn::Error::new(field.span(), "Unknown input property")),
        }

        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
    }

    Ok(InputComponent {
        field_name,
        style: style.ok_or_else(|| syn::Error::new(input.span(), "Missing style"))?,
        label: label.ok_or_else(|| syn::Error::new(input.span(), "Missing label"))?,
        description,
        min_length,
        max_length,
        required,
        placeholder,
    })
}

fn parse_select_component(input: ParseStream) -> Result<SelectComponent> {
    let field_name: Ident = input.parse()?;

    let content;
    syn::braced!(content in input);

    let mut kind = None;
    let mut label = None;
    let mut description = None;
    let mut min_values = None;
    let mut max_values = None;
    let mut required = None;
    let mut placeholder = None;

    while !content.is_empty() {
        let field: Ident = content.parse()?;
        content.parse::<Token![:]>()?;

        match field.to_string().as_str() {
            "kind" => {
                kind = Some(content.parse::<Expr>()?);
            }
            "label" => {
                label = Some(content.parse::<Expr>()?);
            }
            "description" => {
                description = Some(content.parse::<Expr>()?);
            }
            "min_values" => {
                min_values = Some(content.parse::<Expr>()?);
            }
            "max_values" => {
                max_values = Some(content.parse::<Expr>()?);
            }
            "required" => {
                required = Some(content.parse::<Expr>()?);
            }
            "placeholder" => {
                placeholder = Some(content.parse::<Expr>()?);
            }
            _ => return Err(syn::Error::new(field.span(), "Unknown select property")),
        }

        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
    }

    Ok(SelectComponent {
        field_name,
        kind: kind.ok_or_else(|| syn::Error::new(input.span(), "Missing kind"))?,
        label: label.ok_or_else(|| syn::Error::new(input.span(), "Missing label"))?,
        description,
        min_values,
        max_values,
        required,
        placeholder,
    })
}

fn parse_text_component(input: ParseStream) -> Result<TextComponent> {
    let content;
    syn::braced!(content in input);

    let mut text = None;

    while !content.is_empty() {
        let field: Ident = content.parse()?;
        content.parse::<Token![:]>()?;

        match field.to_string().as_str() {
            "content" => {
                text = Some(content.parse::<Expr>()?);
            }
            _ => return Err(syn::Error::new(field.span(), "Unknown select property")),
        }

        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
    }

    Ok(TextComponent {
        content: text.ok_or_else(|| syn::Error::new(input.span(), "Missing content"))?,
    })
}

fn generate_modal_code(modal_def: ModalDefinition) -> Result<TokenStream2> {
    let modal_name = &modal_def.name;
    let modal_custom_id = &modal_def.custom_id;
    let modal_title = &modal_def.title;

    let validated_name = Ident::new(&format!("{}Validated", modal_name), modal_name.span());

    let prefill_parameters = generate_prefill_parameters(&modal_def.components);
    let validated_fields = generate_validated_fields(&modal_def.components);
    let create_components = generate_create_components(&modal_def.components, false);
    let create_components_prefill = generate_create_components(&modal_def.components, true);
    let validation_logic = generate_validation_logic(&modal_def.components, &validated_name);

    // TODO: reduce excessive full paths in macro
    // - don't pollute caller's scope
    // - resolve passed expressions in caller's scope (-> can't use a module scope)

    Ok(quote! {
        pub struct #modal_name;

        #[derive(Clone, Debug)]
        pub struct #validated_name {
            #(#validated_fields,)*
        }

        impl #modal_name {
            pub fn create<'a>(prefix: &'a str) -> ::poise::serenity_prelude::CreateModal<'a>{
                let mut components = Vec::new();

                #(#create_components)*

                ::poise::serenity_prelude::CreateModal::new(format!("{}:{}", prefix, #modal_custom_id), #modal_title)
                    .components(components)
            }

            pub fn create_prefilled<'a>(prefix: &'a str, #(#prefill_parameters,)*) -> ::poise::serenity_prelude::CreateModal<'a> {
                let mut components = Vec::new();

                #(#create_components_prefill)*

                ::poise::serenity_prelude::CreateModal::new(format!("{}:{}", prefix, #modal_custom_id), #modal_title)
                    .components(components)
            }

            pub fn validate<'a>(components: &[::poise::serenity_prelude::ModalComponent]) -> ::anyhow::Result<#validated_name> {
                #validation_logic
            }
        }
    })
}

fn generate_prefill_parameters(components: &[ModalComponent]) -> Vec<TokenStream2> {
    components
        .iter()
        .filter_map(|comp| {
            match comp {
                ModalComponent::Input(input) => {
                    let field_name = &input.field_name;
                    Some(quote! { #field_name: ::std::borrow::Cow<'a, str> })
                }
                ModalComponent::Select(_) => {
                    // no per-instance customisation supported yet
                    None
                }
                ModalComponent::Text(_) => {
                    // doesn't support prefilled values
                    None
                }
            }
        })
        .collect()
}

fn generate_validated_fields(components: &[ModalComponent]) -> Vec<TokenStream2> {
    components
        .iter()
        .filter_map(|comp| {
            match comp {
                ModalComponent::Input(input) => {
                    let field_name = &input.field_name;
                    Some(quote! { pub #field_name: ::poise::serenity_prelude::small_fixed_array::FixedString<u16> })
                }
                ModalComponent::Select(select) => {
                    let field_name = &select.field_name;
                    let kind_variant = extract_select_kind_variant(&select.kind);

                    let field_type = match kind_variant {
                        SelectKindVariant::String => quote! { ::poise::serenity_prelude::small_fixed_array::FixedArray<String> },
                        SelectKindVariant::User => quote! { ::poise::serenity_prelude::small_fixed_array::FixedArray<::poise::serenity_prelude::UserId> },
                        SelectKindVariant::Role => quote! { ::poise::serenity_prelude::small_fixed_array::FixedArray<::poise::serenity_prelude::RoleId> },
                        SelectKindVariant::Mentionable => quote! { ::poise::serenity_prelude::small_fixed_array::FixedArray<::poise::serenity_prelude::GenericId> },
                        SelectKindVariant::Channel => quote! { ::poise::serenity_prelude::small_fixed_array::FixedArray<::poise::serenity_prelude::GenericChannelId> },
                        SelectKindVariant::Unknown => quote! { ::poise::serenity_prelude::SelectMenuValues }, // fallback to enum
                    };

                    Some(quote! { pub #field_name: #field_type })
                }
                // TextDisplay doesn't receive input data
                ModalComponent::Text(_) => None,
            }
        })
        .collect()
}

fn generate_create_components(components: &[ModalComponent], prefill: bool) -> Vec<TokenStream2> {
    components
        .iter()
        .map(|comp| match comp {
            ModalComponent::Input(input) => {
                let field_name = &input.field_name;
                let field_name_str = field_name.to_string();
                let label = &input.label;
                let style = &input.style;

                let mut input_builder = quote! {
                    ::poise::serenity_prelude::CreateInputText::new(#style, #field_name_str)
                };

                if prefill {
                    input_builder = quote! {
                        #input_builder.value(#field_name)
                    }
                }

                if let Some(min_length) = &input.min_length {
                    input_builder = quote! {
                        #input_builder.min_length(#min_length)
                    };
                }

                if let Some(max_length) = &input.max_length {
                    input_builder = quote! {
                        #input_builder.max_length(#max_length)
                    };
                }

                if let Some(required) = &input.required {
                    input_builder = quote! {
                        #input_builder.required(#required)
                    };
                }

                if let Some(placeholder) = &input.placeholder {
                    input_builder = quote! {
                        #input_builder.placeholder(#placeholder)
                    };
                }

                let mut label_builder = quote! {
                        ::poise::serenity_prelude::CreateLabel::input_text(
                            #label,
                            #input_builder,
                        )
                };

                if let Some(description) = &input.description {
                    label_builder = quote! {
                        #label_builder.description(#description)
                    };
                }

                quote! {
                    components.push(
                        ::poise::serenity_prelude::CreateModalComponent::Label(#label_builder)
                    );
                }
            }
            ModalComponent::Select(select) => {
                let field_name = &select.field_name;
                let field_name_str = field_name.to_string();
                let label = &select.label;
                let kind = &select.kind;

                let mut select_builder = quote! {
                    ::poise::serenity_prelude::CreateSelectMenu::new(#field_name_str, #kind)
                };

                if let Some(min_values) = &select.min_values {
                    select_builder = quote! {
                        #select_builder.min_values(#min_values)
                    };
                }

                if let Some(max_values) = &select.max_values {
                    select_builder = quote! {
                        #select_builder.max_values(#max_values)
                    };
                }

                if let Some(required) = &select.required {
                    select_builder = quote! {
                        #select_builder.required(#required)
                    };
                }

                if let Some(placeholder) = &select.placeholder {
                    select_builder = quote! {
                        #select_builder.placeholder(#placeholder)
                    };
                }

                let mut label_builder = quote! {
                        ::poise::serenity_prelude::CreateLabel::select_menu(
                            #label,
                            #select_builder,
                        )
                };

                if let Some(description) = &select.description {
                    label_builder = quote! {
                        #label_builder.description(#description)
                    };
                }

                quote! {
                    components.push(
                        ::poise::serenity_prelude::CreateModalComponent::Label(#label_builder)
                    );
                }
            }
            ModalComponent::Text(text) => {
                let content = &text.content;

                let text_builder = quote! {
                    ::poise::serenity_prelude::CreateTextDisplay::new(#content)
                };

                quote! {
                    components.push(
                        ::poise::serenity_prelude::CreateModalComponent::TextDisplay(#text_builder)
                    );
                }
            }
        })
        .collect()
}

fn generate_validation_logic(
    expected_components: &[ModalComponent],
    validated_name: &Ident,
) -> TokenStream2 {
    let value_extractions: Vec<TokenStream2> = expected_components.iter().filter_map(|comp| {
        match comp {
            ModalComponent::Input(input) => {
                let field_name = &input.field_name;
                let field_name_str = field_name.to_string();

                Some(quote! {
                    let #field_name = {
                        let mut found_value = None;

                        for component in components {
                            if let ::poise::serenity_prelude::ModalComponent::Label(::poise::serenity_prelude::Label {
                                component: ::poise::serenity_prelude::LabelComponent::InputText(input),
                                ..
                            }) = &component {
                                if input.custom_id == #field_name_str {
                                    found_value = Some(input.value.clone().unwrap_or_default());
                                    break;
                                }
                            }
                        }

                        found_value
                            .ok_or_else(|| ::anyhow::anyhow!("Invalid modal data: Missing input field: {}", #field_name_str))?
                    };
                })
            }
            ModalComponent::Select(select) => {
                let field_name = &select.field_name;
                let field_name_str = field_name.to_string();
                let kind_variant = extract_select_kind_variant(&select.kind);

                // TODO: values no longer parsed by serenity, always FixedArray<String> -> individual parsing required
                let extraction_logic = match kind_variant {
                    SelectKindVariant::String => quote! {
                        select_values
                    },
                    SelectKindVariant::User => quote! {
                        select_values
                            .iter()
                            .map(|id| id.parse().map(::poise::serenity_prelude::UserId::new))
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(
                                |_| ::anyhow::anyhow!("Expected User select menu values for field: {}", #field_name_str),
                            )
                    },
                    SelectKindVariant::Role => quote! {
                        select_values
                            .iter()
                            .map(|id| id.parse().map(::poise::serenity_prelude::RoleId::new))
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(
                                |_| ::anyhow::anyhow!("Expected Role select menu values for field: {}", #field_name_str),
                            )
                    },
                    SelectKindVariant::Mentionable => quote! {
                        select_values
                            .iter()
                            .map(|id| id.parse().map(::poise::serenity_prelude::GenericId::new))
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(
                                |_| ::anyhow::anyhow!("Expected Mentionable select menu values for field: {}", #field_name_str),
                            )
                    },
                    SelectKindVariant::Channel => quote! {
                        select_values
                            .iter()
                            .map(|id| id.parse().map(::poise::serenity_prelude::GenericChannelId::new))
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(
                                |_| ::anyhow::anyhow!("Expected Channel select menu values for field: {}", #field_name_str),
                            )
                    },
                    SelectKindVariant::Unknown => quote! {
                        select_values // fallback to raw strings
                    },
                };

                Some(quote! {
                    let #field_name = {
                        let mut found_value = None;

                        for component in components {
                            if let ::poise::serenity_prelude::ModalComponent::Label(::poise::serenity_prelude::Label {
                                component: ::poise::serenity_prelude::LabelComponent::SelectMenu(select),
                                ..
                            }) = &component {
                                if select.custom_id == #field_name_str {
                                    found_value = Some(select.values.clone());
                                    break;
                                }
                            }
                        }

                        let select_values = found_value
                            .ok_or_else(|| ::anyhow::anyhow!("Invalid modal data: Missing select field: {}", #field_name_str))?;

                        #extraction_logic
                    };
                })
            }
            // TextDisplay doesn't receive input data
            ModalComponent::Text(_) => None
        }
    }).collect();

    let field_names: Vec<&Ident> = expected_components
        .iter()
        .filter_map(|comp| match comp {
            ModalComponent::Input(input) => Some(&input.field_name),
            ModalComponent::Select(select) => Some(&select.field_name),
            ModalComponent::Text(_) => None,
        })
        .collect();

    quote! {
        #(#value_extractions)*

        Ok(#validated_name {
            #(#field_names,)*
        })
    }
}
