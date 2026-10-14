# ADR 0001 — Option B : WordCraft conservé, profil Markdown à la frontière de persistance

- Statut : proposé (14/10/2026)
- Contexte : le cahier des charges impose Markdown comme seul format de sauvegarde, sans perte silencieuse, tout en conservant l'expérience Word de WordCraft. Le modèle `Document` est bien plus riche que Markdown ; un modèle intermédiaire `Flow` existe déjà pour tous les formats texte (`crates/formats/src/model.rs`).
- Décision : ne pas modifier `doc`, `layout`, `render`, `pdf`, `docx`, `docbin`. Ajouter un profil Markdown natif dans `crates/formats/src/markdown.rs` (`export_native`, `import_with`, `Report`) et brancher l'I/O de l'engine dessus.
- Conséquences : l'édition garde toutes les fonctions WordCraft ; les pertes ont lieu à l'enregistrement, une seule fois, et sont rapportées ; certaines commandes devront être signalées ou grisées (mode strict/souple).
- Alternatives écartées : option A (sauvegarde directe : perte silencieuse ou HTML caché), option C (moteur Markdown alternatif : rien ne le justifie).
