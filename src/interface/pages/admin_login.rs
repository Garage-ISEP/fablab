use leptos::prelude::*;

use crate::interface::components::flash_message::FlashMessage;

#[server]
async fn get_login_csrf() -> Result<String, ServerFnError>
{
    crate::interface::session_helpers::get_csrf_token().await
}

#[component]
pub fn AdminLoginPage() -> impl IntoView
{
    let csrf = Resource::new(|| (), |_| get_login_csrf());

    view!
    {
        <div class="login-card">
            <h1>"Administration"</h1>
            <p class="login-subtitle">"Espace reserve a l'equipe Fablab"</p>

            <FlashMessage />

            <Suspense fallback=|| view! { <p class="loading">"Chargement..."</p> }>
                {move ||
                {
                    csrf.get().map(|result|
                    {
                        let token = result.unwrap_or_default();
                        view!
                        {
                            <form method="post" action="/auth/admin/login">
                                <input type="hidden" name="_csrf" value=token />
                                <div class="form-group">
                                    <label for="login">"Identifiant"</label>
                                    <input id="login" name="login" type="text" required placeholder="admin" />
                                </div>
                                <div class="form-group">
                                    <label for="password">"Mot de passe"</label>
                                    <input id="password" name="password" type="password" required />
                                </div>
                                <button type="submit" class="btn btn-primary btn-block">"Se connecter"</button>
                            </form>
                        }
                    })
                }}
            </Suspense>

            <p class="login-back"><a href="/">"Retour a l'accueil"</a></p>
        </div>
    }
}
