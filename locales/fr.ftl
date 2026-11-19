# Les messages français de l'application. Le catalogue de Tablo contient le reste.

## Pages

page-home = Accueil

## Refus

error-email-taken = Cette adresse e-mail est déjà utilisée.
error-name-required = Saisissez votre nom.
error-email-invalid = Saisissez une adresse e-mail valide.
error-team-name-required = Saisissez un nom d'équipe.
error-owner-by-transfer = Le propriétaire ne change que par transfert de propriété.
error-already-member = Cette personne fait déjà partie de l'équipe.
error-transfer-one = Transférez la propriété à un seul membre à la fois.
error-no-user = Reconnectez-vous.
error-no-team = Choisissez d'abord une équipe.

## Paramètres

settings-account = Compte
settings-security = Sécurité
settings-general = Général
invitations-waiting = { $count ->
    [one] Une invitation à rejoindre une équipe vous attend.
   *[other] { $count } invitations à rejoindre une équipe vous attendent.
}

## Les tenants de Tablo, que cette application appelle équipes

-tenant = { $form ->
   *[lower] équipe
    [upper] Équipe
    [upper-plural] Équipes
    [indefinite] une équipe
    [definite] l'équipe
    [of] d'équipe
    [of-definite] de l'équipe
    [another] une autre équipe
}
    .gender = feminine

## Équipes

team-personal = Équipe de { $name }
flash-invitation-sent = Invitation envoyée à { $email }.

## E-mails

mail-invitation-subject = Rejoignez { $team }
mail-invitation-title = Vous êtes invité à rejoindre { $team }
mail-invitation-body = { $inviter } vous invite à rejoindre { $team }. Connectez-vous avec cette adresse pour accepter.
mail-invitation-action = Voir votre invitation

## Facturation

page-billing = Facturation
plan-free = Gratuit
plan-pro = Pro
plan-business = Business
billing-description = Votre offre, vos sièges et vos factures.
billing-current = Offre actuelle
billing-seats = { $used } sièges
billing-seats-limited = { $used } sièges sur { $limit }
error-no-billing-account = Cette équipe n'a pas encore de compte de facturation. Choisissez d'abord une offre.
error-billing-unavailable = La facturation est indisponible pour le moment. Réessayez dans une minute.
error-plan-invalid = Choisissez l'une des offres proposées.
billing-off = La facturation n'est pas configurée sur ce serveur. Renseignez les variables STRIPE_ pour l'activer.
billing-per-seat = { $price } par siège et par mois
billing-monthly = Facturé chaque mois
billing-yearly = Facturé chaque année, { $price } par siège
billing-invoices = Factures
billing-invoice-open = Ouvrir
billing-trialing = Votre essai court jusqu'au { $date }.
billing-ending = Votre abonnement prend fin le { $date }.
billing-grace = Votre dernier paiement a échoué. Mettez à jour votre carte avant le { $date } pour conserver votre offre.
billing-lapsed = Votre abonnement a expiré : cette équipe est passée à l'offre Gratuit. Rien n'a été supprimé.
error-seat-limit = Votre offre n'a plus de siège disponible. Passez à l'offre supérieure pour inviter d'autres personnes.
error-team-full = Cette équipe n'a plus de siège disponible. Demandez à son propriétaire de changer d'offre.
mail-payment-failed-subject = Votre paiement a échoué
mail-payment-failed-body = Nous n'avons pas pu débiter votre carte. Nous réessaierons, mais mettez-la à jour pour conserver votre offre.
mail-trial-ending-subject = Votre essai se termine bientôt
mail-trial-ending-body = Votre essai se termine dans quelques jours. Votre abonnement démarrera alors, sauf si vous l'annulez.
mail-billing-action = Ouvrir la facturation

## Tableau de bord et projets

dashboard-seats = Sièges
dashboard-projects = Projets
error-project-limit = Votre offre n'a plus de projet disponible. Passez à l'offre supérieure pour en créer un autre.

## Suppression

error-team-name-mismatch = Saisissez exactement le nom de l'équipe.
error-password-wrong = Ce mot de passe est incorrect.
error-transfer-first = Vous êtes propriétaire d'une équipe utilisée par d'autres. Transférez-la avant de supprimer votre compte.

## Support

page-support = Support
support-description = Trouvez un client et agissez en son nom pour voir ce qu'il voit. Cela dure { $minutes } minutes au plus, et le back-office consigne qui a agi au nom de qui.
support-needs-factor = Agir au nom d'un client demande un second facteur ici, dans l'application, configuré avant qu'un administrateur ne vous accorde votre rôle interne. Configurez-en un, puis redemandez ce rôle.
support-teams = Équipes
support-act-as = Agir au nom de ce client
support-act-as-one = Agissez au nom d'un seul client à la fois.
support-acting = Vous agissez désormais au nom de ce client.
impersonation-entry = Session au nom d'un client
impersonation-log = Sessions au nom de clients
impersonation-staff = Membre de l'équipe interne
impersonation-event = Événement
impersonation-started = Début
impersonation-stopped = Fin
impersonation-until = Prend fin au plus tard
impersonation-customer = Client
impersonation-customer-id = Identifiant du client

## Site

site-nav = Site
site-pricing = Tarifs
site-sign-in = Connexion
site-sign-up = Inscription
site-source = Code source
site-terms = Conditions
site-privacy = Confidentialité
site-kicker = Un modèle de SaaS en Rust, prêt pour la production
site-title-1 = Forkez le modèle.
site-title-2 = Remplacez un dossier.
site-lead = Des équipes comme locataires, des abonnements Stripe par siège, une file de tâches dans Postgres, la sécurité des comptes et un back-office pour l'équipe, sur Tablo, Topcoat et Toasty. Un domaine d'exemple tient la place de votre produit ; le reste, vous le gardez.
site-live-note = Ce site est le modèle tel qu'il est livré. Inscrivez-vous, et l'application derrière est à vous pour l'essayer.
site-start = Commencer gratuitement
site-read-source = Lire le code
site-meta-locales = Anglais et français
site-panels-title = Deux panneaux
site-panels-note = Et l'entrée de chacun
site-panel-app-title = L'application
site-panel-app-body = Là où travaillent les clients : leurs équipes et membres, l'offre qu'ils paient et les projets d'exemple. Tout le monde peut s'inscrire.
site-panel-app-sign-up = Créer un compte
site-panel-admin-title = Back-office
site-panel-admin-body = Là où travaille l'équipe, à part de l'application et derrière un second facteur. Le rôle s'accorde en ligne de commande au déploiement, jamais depuis une page :
site-parts-title = Ce que vous gardez
site-parts-note = Et où cela se trouve
site-col-part = Partie
site-col-does = Ce qu'elle fait
site-col-code = Dans le code
site-part-teams = Équipes
site-part-teams-body = Chaque compte reçoit une équipe personnelle. Propriétaires, administrateurs et membres, invitations, transfert de propriété, et un seul propriétaire, que la base garantit.
site-part-billing = Facturation
site-part-billing-body = Les offres Free, Pro et Business, définies dans le code et vendues par siège via Stripe Checkout et le portail client, avec essais, taxes, prorata et sept jours de grâce après un paiement refusé.
site-part-webhooks = Webhooks
site-part-webhooks-body = Une boîte de réception signée. Les événements en double ou dans le désordre relisent l'état chez Stripe, et une réconciliation passe toutes les six heures.
site-part-jobs = Tâches
site-part-jobs-body = Une file durable dans Postgres : voies, dédoublonnage, reprises espacées, un état mort, des planifications, et une boîte d'envoi, pour qu'une tâche existe exactement quand son écriture est validée.
site-part-accounts = Comptes
site-part-accounts-body = Vérification de l'adresse, réinitialisation du mot de passe, double authentification par application ou par e-mail, « Continuer avec Google », suppression du compte, et un téléchargement JSON de tout ce qu'un compte possède.
site-part-backoffice = Back-office
site-part-backoffice-body = Un second panneau, à part de l'application, avec la double authentification exigée du personnel : comptes, équipes, abonnements, événements de webhook et tâches, et un journal de ce que le personnel a fait.
site-part-support = Support
site-part-support-body = Le personnel voit les pages d'un client comme lui, 30 minutes au plus et via un second facteur, sans toucher à ses identifiants, à ses équipes ni à son argent.
site-part-audit = Audit
site-part-audit-body = Le journal d'audit de Tablo, écrit dans la transaction de chaque écriture avec les champs modifiés : celui d'une équipe avec l'offre Business, et celui du back-office.
site-part-product = Votre produit
site-part-product-body = Le domaine d'exemple : une ressource limitée à l'équipe, un formulaire, une limite d'offre vérifiée dans la transaction de l'écriture, et une tuile de tableau de bord. Reprenez sa forme, puis supprimez-le.
site-replace = À remplacer
site-plans-note = Lus dans le catalogue des offres
site-compare = Comparer les offres
site-run-title = Le lancer
site-run-note = Trois commandes
site-run-body = Il vous faut Rust, Docker et la CLI topcoat. L'application répond sur 127.0.0.1:3000, et les e-mails qu'elle envoie sont interceptés sur 127.0.0.1:8026.
site-run-billing = Sans les variables Stripe, la facturation est désactivée : chaque équipe reste sur l'offre gratuite.
site-run-env = la configuration, depuis l'exemple
site-run-services = Postgres, une imitation de Stripe et un intercepteur d'e-mails
site-run-app = l'application, en développement
site-pricing-title-1 = Payez par siège.
site-pricing-title-2 = Commencez gratuitement.
site-pricing-body = Les offres sont dans src/billing/plans.rs. Cette page les lit et le catalogue Stripe en est créé : la page ne peut pas contredire ce qui est facturé.
site-col-plan = Offre
site-col-price = Prix
site-col-seats = Sièges
site-col-projects = Projets
site-col-audit = Journal d'audit de l'équipe
site-col-support = Support prioritaire
site-included = Inclus
site-not-included = Non inclus
site-price-free = Gratuit pour toujours
site-unlimited = Illimité
site-terms-body = Texte d'exemple. Remplacez-le par des conditions validées par votre juriste avant le lancement.
site-privacy-body = Texte d'exemple. Remplacez-le par une politique conforme aux données que vous collectez avant le lancement.
site-foot-template = Modèle
site-foot-built-on = Construit sur
site-foot-more = Aussi
site-tablo-site = Site de Tablo
site-guide = Guide de Tablo
site-demo = Démo de Tablo
site-examples = Galerie d'exemples
site-built-with = Construit avec Tablo, Topcoat et Toasty.

## Libellés que Tablo cherche par convention

resource-project = Projet
    .plural = Projets
resource-member = Membre
    .plural = Membres
resource-invitation = Invitation en attente
    .plural = Invitations en attente
field-name = Nom
field-email = E-mail
field-password = Mot de passe
field-role = Rôle
field-interval = Facturation
field-status = Statut
field-description = Description
field-user = Personne
field-actor = Qui
field-at = Quand
field-resource = Quoi
field-change = Modification
action-manage-billing = Gérer la facturation
action-checkout = S'abonner
action-invite = Inviter quelqu'un
action-change-role = Changer le rôle
action-remove = Retirer
action-transfer-ownership = Transférer la propriété
action-leave = Quitter l'équipe
action-revoke = Révoquer
action-accept = Accepter
action-decline = Refuser
action-delete-team = Supprimer cette équipe
action-export-account = Télécharger mes données
action-delete-account = Supprimer mon compte
hint-resync = Relire cet abonnement depuis Stripe
hint-retry = Rendre ses tentatives à cette tâche
filter-staff = Équipe interne
filter-customers = Clients
filter-verified = Vérifiés
filter-unverified = Non vérifiés
filter-processed = Traités
filter-pending = Pas encore traités

field-created_at = Créé le
field-expires_at = Expire le
field-invited_by = Invité par
field-team_name = Équipe
field-password_confirmation = Confirmez le mot de passe
field-staff_role = Rôle interne
field-customer_id = Client Stripe
field-plan = Offre
field-seats = Sièges
field-current_period_end = Renouvellement
field-synced_at = Dernière synchronisation
field-received_at = Reçu le
field-processed_at = Traité le
field-kind = Type
field-attempts = Tentatives
field-run_at = Exécution
field-last_error = Dernière erreur
resource-account = Compte
    .plural = Comptes
resource-customer = Équipe
    .plural = Équipes
resource-subscription = Abonnement
    .plural = Abonnements
resource-webhook = Webhook
    .plural = Webhooks
resource-job = Tâche
    .plural = Tâches
resource-incoming-invitation = Invitation
    .plural = Invitations
option-team-role =
    .owner = Propriétaire
    .admin = Administrateur
    .member = Membre
option-project-status =
    .active = Actif
    .paused = En pause
    .done = Terminé
option-staff-role =
    .administrator = Administrateur
    .support = Support
option-billing-status =
    .none = Sans abonnement
    .trialing = Essai
    .active = Actif
    .past_due = Paiement en retard
    .unpaid = Impayé
    .canceled = Résilié
    .incomplete = Incomplet
    .incomplete_expired = Expiré
    .paused = En pause
option-plan-id =
    .free = Gratuit
    .pro = Pro
    .business = Business
option-billing-interval =
    .month = Mensuel
    .year = Annuel
option-job-state =
    .queued = En attente
    .running = En cours
    .succeeded = Réussi
    .dead = Abandonné
