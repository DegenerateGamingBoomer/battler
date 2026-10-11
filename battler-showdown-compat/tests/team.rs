use battler_champions::{
    Regulation,
    legality::Legality,
};
use battler_showdown_compat::{
    names,
    species::resolve,
    team::{
        ConvertOptions,
        convert_text,
        parse_exported,
        parse_packed,
    },
};
use battler_test_utils::static_local_data_store;

#[test]
fn every_champions_species_resolves() {
    let data = static_local_data_store();
    let legality = Legality::for_regulation(Regulation::MC).unwrap();
    let mut unresolved = Vec::new();
    let mut cosmetic = 0;
    for id in names::all_species_ids() {
        if !legality.is_species_legal(&battler_data::Id::from(id)) {
            continue;
        }
        match resolve(data, id) {
            Ok(species) => cosmetic += species.cosmetic as usize,
            Err(_) => unresolved.push(id),
        }
    }
    assert!(unresolved.is_empty(), "{unresolved:?}");
    assert!(cosmetic > 0);
}

#[test]
fn resolves_renamed_formes() {
    let data = static_local_data_store();
    let name = |id: &str| resolve(data, id).unwrap().name;
    assert_eq!(name("indeedeef"), "Indeedee-Female");
    assert_eq!(name("taurospaldeablaze"), "Tauros-Paldea-Blaze-Breed");
    assert_eq!(name("squawkabillywhite"), "Squawkabilly-White-Plumage");
    assert_eq!(name("mausholdfour"), "Maushold");
    assert_eq!(name("garchomp"), "Garchomp");
    let alcremie = resolve(data, "alcremierubycream").unwrap();
    assert_eq!(alcremie.name, "Alcremie");
    assert_eq!(alcremie.showdown_name, "Alcremie-Ruby-Cream");
    assert!(alcremie.cosmetic);
}

#[test]
fn parses_the_packed_format() {
    let packed = "Pikachu||ChoiceScarf|Static|thunderbolt,protect|Timid|,,,252,4,252|F||S|50|120,,,,,\
        ]Mr. Mime||leftovers|0|psychic|Hardy|||0,,,,,|M||50|";
    let sets = parse_packed(packed).unwrap();
    assert_eq!(sets.len(), 2);
    assert_eq!(sets[0].species, "Pikachu");
    assert_eq!(sets[0].moves, ["thunderbolt", "protect"]);
    assert_eq!(sets[0].evs, [0, 0, 0, 252, 4, 252]);
    assert_eq!(sets[0].ivs, [31; 6]);
    assert!(sets[0].shiny);
    assert_eq!(sets[0].level, Some(50));
    assert_eq!(sets[0].happiness, Some(120));
    assert_eq!(sets[1].ivs, [0, 31, 31, 31, 31, 31]);
}

#[test]
fn parses_the_exported_format() {
    let text = "Sparky (Pikachu) (F) @ Choice Scarf\nAbility: Static\nLevel: 50\nShiny: Yes\n\
        EVs: 252 SpA / 4 SpD / 252 Spe\nTimid Nature\nIVs: 0 Atk\n- Thunderbolt\n- Protect\n\n\
        Charizard @ Life Orb\nAbility: Blaze\n- Flamethrower\n";
    let sets = parse_exported(text).unwrap();
    assert_eq!(sets.len(), 2);
    assert_eq!(sets[0].name, "Sparky");
    assert_eq!(sets[0].species, "Pikachu");
    assert_eq!(sets[0].gender, "F");
    assert_eq!(sets[0].item, "Choice Scarf");
    assert_eq!(sets[0].evs, [0, 0, 0, 252, 4, 252]);
    assert_eq!(sets[0].ivs, [31, 0, 31, 31, 31, 31]);
    assert_eq!(sets[0].nature, "Timid");
    assert!(sets[0].shiny);
    assert_eq!(sets[1].name, "Charizard");
    assert_eq!(sets[1].species, "Charizard");
}

#[test]
fn converts_to_battler_teams() {
    let data = static_local_data_store();
    let packed = "Sparky|Pikachu|choicescarf|Static|thunderbolt,protect|Timid|,,,32,2,32|F||S|50|";
    let team = convert_text(data, packed, ConvertOptions::default()).unwrap();
    let member = &team.team.members[0];
    assert_eq!(member.name, "Sparky");
    assert_eq!(member.species, "Pikachu");
    assert_eq!(member.level, 50);
    assert_eq!(member.evs.spa, 32);
    assert_eq!(member.ivs.hp, 31);
    assert!(member.shiny);
    assert_eq!(member.item.as_deref(), Some("choicescarf"));

    let forced = convert_text(
        data,
        packed,
        ConvertOptions {
            force_level: Some(50),
            default_level: 100,
        },
    )
    .unwrap();
    assert_eq!(forced.team.members[0].level, 50);
    let defaulted = convert_text(
        data,
        "Pikachu||||thunderbolt|||||||",
        ConvertOptions::default(),
    );
    assert_eq!(defaulted.unwrap().team.members[0].level, 100);
}
