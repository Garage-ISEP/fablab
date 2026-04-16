use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;

use fablab::interface::components::nav::Nav;
use fablab::interface::pages::admin_dashboard::AdminDashboardPage;
use fablab::interface::pages::admin_login::AdminLoginPage;
use fablab::interface::pages::admin_materials::AdminMaterialsPage;
use fablab::interface::pages::admin_order_edit::AdminOrderEditPage;
use fablab::interface::pages::home::HomePage;
use fablab::interface::pages::my_order_detail::MyOrderDetailPage;
use fablab::interface::pages::my_orders::MyOrdersPage;
use fablab::interface::pages::order_form::OrderFormPage;

pub fn shell() -> impl IntoView
{
    view!
    {
        <!DOCTYPE html>
        <html lang="fr">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <link rel="preconnect" href="https://fonts.googleapis.com" />
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
                <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet" />
                <MetaTags />
            </head>
            <body>
                <App />
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView
{
    provide_meta_context();

    view!
    {
        <Stylesheet id="leptos" href="/style/main.css" />
        <Title text="Fablab | Garage Isep" />
        <Router>
            <Nav />
            <main class="content">
                <Routes fallback=||
                    view!
                    {
                        <div class="page-card centered">
                            <h1>"404"</h1>
                            <p>"Page introuvable"</p>
                            <a href="/" class="btn btn-primary">"Retour a l'accueil"</a>
                        </div>
                    }
                >
                    <Route path=path!("/") view=HomePage />
                    <Route path=path!("/order/new") view=OrderFormPage />
                    <Route path=path!("/my-orders") view=MyOrdersPage />
                    <Route path=path!("/my-orders/:id") view=MyOrderDetailPage />
                    <Route path=path!("/admin") view=AdminDashboardPage />
                    <Route path=path!("/admin/login") view=AdminLoginPage />
                    <Route path=path!("/admin/order/:id") view=AdminOrderEditPage />
                    <Route path=path!("/admin/materials") view=AdminMaterialsPage />
                </Routes>
            </main>
            <footer class="footer">
                <p>
                    "Fablab Garage Isep | Impression 3D - "
                    <a href="mailto:fablab@garageisep.com" class="footer-link">"Contact : fablab@garageisep.com"</a>
                </p>
            </footer>
        </Router>
    }
}
