use leptos::prelude::*;

#[server]
pub async fn fetch_csrf_token() -> Result<String, ServerFnError>
{
    crate::interface::session_helpers::get_csrf_token().await
}

#[component]
pub fn CsrfField() -> impl IntoView
{
    let token = Resource::new(|| (), |_| fetch_csrf_token());

    view!
    {
        <Suspense fallback=|| view! { <input type="hidden" name="_csrf" value="" /> }>
            {move ||
            {
                token.get().map(|result|
                {
                    let value = result.unwrap_or_default();
                    view! { <input type="hidden" name="_csrf" value=value /> }
                })
            }}
        </Suspense>
    }
}
