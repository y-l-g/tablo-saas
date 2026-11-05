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

## Navigation

navigation-team = Équipe

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
billing-manage = Gérer la facturation
billing-off = La facturation n'est pas configurée sur ce serveur. Renseignez les variables STRIPE_ pour l'activer.
billing-per-seat = { $price } par siège et par mois
billing-choose-monthly = Choisir le mensuel
billing-choose-yearly = Choisir l'annuel, { $price } par siège
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

page-account = Votre compte
account-description = Supprimer votre compte supprime les équipes dont vous êtes le seul membre et résilie leurs abonnements. Cette action est irréversible.
error-team-name-mismatch = Saisissez exactement le nom de l'équipe.
error-password-wrong = Ce mot de passe est incorrect.
error-transfer-first = Vous êtes propriétaire d'une équipe utilisée par d'autres. Transférez-la avant de supprimer votre compte.

## Site

site-pricing = Tarifs
site-sign-in = Connexion
site-sign-up = Inscription
site-terms = Conditions
site-privacy = Confidentialité
site-headline = Pilotez toute votre équipe depuis un seul endroit.
site-lead = Tout ce dont une entreprise a besoin dès le premier jour : équipes, abonnements et back-office.
site-start = Commencer gratuitement
site-feature-teams = Des équipes avec rôles, invitations et journal d'audit.
site-feature-billing = Abonnements, sièges et factures avec Stripe.
site-feature-security = Adresses vérifiées et connexion à deux facteurs.
site-price-free = Gratuit pour toujours
site-unlimited = Illimité
site-plan-seats = Sièges : { $seats }
site-plan-projects = Projets : { $projects }
site-terms-body = Texte d'exemple. Remplacez-le par des conditions validées par votre juriste avant le lancement.
site-privacy-body = Texte d'exemple. Remplacez-le par une politique conforme aux données que vous collectez avant le lancement.
audit-deleted-actor = Un compte supprimé

## Libellés que Tablo cherche par convention

resource-project = Projet
    .plural = Projets
resource-member = Membre
    .plural = Membres
resource-invitation = Invitation
    .plural = Invitations
resource-audit = Entrée d'audit
    .plural = Journal d'audit
field-name = Nom
field-email = E-mail
field-password = Mot de passe
field-role = Rôle
field-status = Statut
field-description = Description
field-user = Personne
field-actor = Qui
field-at = Quand
field-resource = Quoi
field-change = Modification
action-invite = Inviter quelqu'un
action-change-role = Changer le rôle
action-remove = Retirer
action-transfer-ownership = Transférer la propriété
action-leave = Quitter l'équipe
action-revoke = Révoquer
action-accept = Accepter
action-decline = Refuser
action-delete-team = Supprimer cette équipe
action-delete-account = Supprimer mon compte

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
resource-incoming-invitation = Invitation pour vous
    .plural = Invitations pour vous
resource-staff-audit = Entrée d'audit interne
    .plural = Journal d'audit interne
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
