# ADR 0003 — Connecteurs de forge par API REST, Git local par gix

- Statut : proposé (14/10/2026)
- Contexte : un seul fichier Markdown ouvert à la fois ; GitHub, GitLab, Forgejo ; version Web en WASM sans système de fichiers ni processus.
- Décision : pour les dépôts distants, lire/écrire le fichier et lister son historique par les API REST des forges (un commit par « Enregistrer une version », contrôle de concurrence par `sha` attendu). Pour les dépôts locaux (desktop), `gix` ; la commande `git` en repli pour push/pull SSH. Le transport HTTP et le stockage des secrets sont des services injectés par l'application ; `engine` reste sans réseau.
- Conséquences : même comportement desktop et Web ; pas de dépendance C ; CORS et authentification à traiter pour le Web ; pas d'opérations multi-fichiers complexes (hors besoin).
