use leptos::prelude::*;

#[server]
pub async fn get_flash() -> Result<Option<(String, String)>, ServerFnError>
{
    use tower_sessions::Session;
    use crate::interface::flash::take_flash;

    let session: Session = leptos_axum::extract().await?;
    take_flash(&session)
        .await
        .map_err(|e| ServerFnError::new(format!("{e}")))
}

#[component]
pub fn FlashMessage() -> impl IntoView
{
    let flash = Resource::new(|| (), |_| get_flash());

    view!
    {
        <Suspense fallback=|| ()>
            {move ||
            {
                flash.get().and_then(|result|
                {
                    match result
                    {
                        Ok(Some((level, message))) =>
                        {
                            let class = format!("flash-message flash-{level}");
                            Some(view!
                            {
                                <div class=class role="alert">
                                    <span class="flash-text">{message}</span>
                                </div>
                            })
                        }
                        _ => None,
                    }
                })
            }}
        </Suspense>
    }
}
