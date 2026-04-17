use leptos::prelude::*;

use crate::interface::components::csrf_field::CsrfField;
use crate::interface::pages::home::get_session_info;

#[component]
pub fn Nav() -> impl IntoView
{
    let session = Resource::new(|| (), |_| get_session_info());

    view!
    {
        <header class="navbar">
            <div class="navbar-inner">
                <a href="/" class="navbar-brand">
                    <span class="brand-icon">
                        <img src="/public/logo_fablab.svg" alt="Fablab" />
                    </span>
                    "Fablab Garage Isep"
                </a>
                <Suspense fallback=|| view! { <nav class="navbar-links"></nav> }>
                    {move ||
                    {
                        session.get().map(|result|
                        {
                            match result
                            {
                                Ok(Some((role, _))) if role == "admin" =>
                                {
                                    view!
                                    {
                                        <nav class="navbar-links">
                                            <a href="/admin">"Commandes"</a>
                                            <a href="/admin/materials">"Materiaux"</a>
                                            <form method="post" action="/auth/logout" class="nav-form">
                                                <CsrfField />
                                                <button type="submit" class="btn-nav">"Deconnexion"</button>
                                            </form>
                                        </nav>
                                    }.into_any()
                                }
                                Ok(Some((_, _))) =>
                                {
                                    view!
                                    {
                                        <nav class="navbar-links">
                                            <a href="/order/new">"Nouvelle commande"</a>
                                            <a href="/my-orders">"Mes commandes"</a>
                                            <form method="post" action="/auth/logout" class="nav-form">
                                                <CsrfField />
                                                <button type="submit" class="btn-nav">"Deconnexion"</button>
                                            </form>
                                        </nav>
                                    }.into_any()
                                }
                                _ =>
                                {
                                    view!
                                    {
                                        <nav class="navbar-links">
                                            <a href="/auth/cas" class="btn-nav-primary">"Connexion Isep"</a>
                                        </nav>
                                    }.into_any()
                                }
                            }
                        })
                    }}
                </Suspense>
            </div>
        </header>
    }
}
