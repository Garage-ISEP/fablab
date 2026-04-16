use leptos::prelude::*;

#[component]
pub fn StatusBadge(status: String) -> impl IntoView
{
    let (label, class) = match status.as_str()
    {
        "a_traiter" => ("A traiter", "badge badge-pending"),
        "en_traitement" => ("En traitement", "badge badge-processing"),
        "imprime" => ("Imprime", "badge badge-printed"),
        "livre" => ("Livre", "badge badge-delivered"),
        "annule" => ("Annule", "badge badge-cancelled"),
        _ => ("Inconnu", "badge"),
    };

    view! { <span class=class>{label}</span> }
}
