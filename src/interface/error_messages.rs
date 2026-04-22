use crate::application::errors::AppError;

pub fn user_message(err: &AppError) -> &'static str
{
    match err
    {
        AppError::NotAuthorized => "Acces refuse. Veuillez vous connecter.",

        AppError::NotFound(s) if s.starts_with("order") =>
            "Commande introuvable. Elle a peut-etre ete supprimee.",
        AppError::NotFound(s) if s.starts_with("material") => "Materiau introuvable.",
        AppError::NotFound(s) if s.starts_with("user") => "Utilisateur introuvable.",
        AppError::NotFound(_) => "Element introuvable.",

        AppError::InvalidInput(s) if s.contains("stock insuffisant") =>
            "Stock insuffisant : le nouveau poids depasse la quantite restante de la bobine.",
        AppError::InvalidInput(s) if s.contains("un materiau doit etre defini") =>
            "Un materiau doit etre selectionne pour faire avancer la commande.",
        AppError::InvalidInput(s) if s.contains("pas disponible") =>
            "Le materiau selectionne n'est pas disponible.",
        AppError::InvalidInput(s) if s.contains("can no longer be cancelled") =>
            "Cette commande ne peut plus etre annulee.",

        AppError::InvalidInput(s) if s.contains("referenced by orders") =>
            "Impossible de supprimer : ce materiau est utilise par des commandes.",

        AppError::InvalidInput(s) if s.contains("status transition") =>
            "Changement de statut impossible pour cette commande.",
        AppError::InvalidInput(s) if s.contains("quantity") =>
            "La quantite doit etre d'au moins 1.",
        AppError::InvalidInput(s) if s.contains("too many files") =>
            "Nombre maximal de fichiers par commande atteint.",
        AppError::InvalidInput(s) if s.contains("too large") =>
            "Fichier trop volumineux.",
        AppError::InvalidInput(s) if s.contains("content does not match") =>
            "Le contenu du fichier ne correspond pas a son extension.",
        AppError::InvalidInput(s) if s.contains("unsupported file type") =>
            "Type de fichier non supporte. Formats acceptes: STL, 3MF, STP.",
        AppError::InvalidInput(s) if s.contains("invalid file name") =>
            "Nom de fichier invalide.",
        AppError::InvalidInput(s) if s.contains("empty file") =>
            "Le fichier est vide.",
        AppError::InvalidInput(s) if s.contains("upload interrupted") =>
            "Envoi interrompu. Veuillez reessayer.",
        AppError::InvalidInput(s) if s.contains("missing file") =>
            "Veuillez joindre au moins un fichier.",
        AppError::InvalidInput(s) if s.contains("software_used") =>
            "Veuillez indiquer le logiciel utilise.",
        AppError::InvalidInput(s) if s.contains("weight") =>
            "Le poids doit etre positif ou nul.",
        AppError::InvalidInput(s) if s.contains("print time") =>
            "Le temps d'impression doit etre positif ou nul.",
        AppError::InvalidInput(s) if s.contains("phone is required") =>
            "Le numero de telephone est obligatoire.",
        AppError::InvalidInput(s) if s.contains("phone") =>
            "Numero de telephone invalide.",
        AppError::InvalidInput(_) => "Donnees invalides. Verifiez votre saisie.",

        AppError::Database(_) => "Erreur interne. Veuillez reessayer.",
    }
}
