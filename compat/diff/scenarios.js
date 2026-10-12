'use strict';
// Scripted differential scenarios (spec P1c, "Scenario tests"), played by
// `node compat/diff/driver.js --scenarios [filter]` on both engines under the semantic streams.
//
// Each scenario has two teams of Showdown sets, the team preview picks and one entry per decision
// step: [p1 choice, p2 choice] in Showdown syntax ('' when a side has nothing to choose).

function set(species, ability, moves, extra = {}) {
  return {
    name: extra.name || species, species, ability, moves, item: extra.item || '',
    gender: extra.gender || 'M', level: 50, nature: extra.nature || 'Hardy',
    evs: extra.evs || {hp: 11, atk: 11, def: 11, spa: 11, spd: 11, spe: 11},
    ivs: {hp: 31, atk: 31, def: 31, spa: 31, spd: 31, spe: 31},
  };
}

const sp = spe => ({hp: 20, atk: 20, def: 0, spa: 20, spd: 0, spe});

function scenarios() {
  return [
    {
      name: 'fake-out-first-turn-only',
      teams: [
        [set('Incineroar', 'Blaze', ['Fake Out', 'Flare Blitz', 'Parting Shot', 'Protect']),
          set('Rillaboom', 'Overgrow', ['Fake Out', 'Wood Hammer', 'Grassy Glide', 'Protect'], {evs: sp(10)}),
          set('Garchomp', 'Rough Skin', ['Earthquake']), set('Gholdengo', 'Good as Gold', ['Shadow Ball'])],
        [set('Kingambit', 'Defiant', ['Sucker Punch', 'Iron Head', 'Protect', 'Swords Dance'], {evs: sp(2)}),
          set('Sinistcha', 'Hospitality', ['Matcha Gotcha', 'Rage Powder', 'Trick Room', 'Protect'], {evs: sp(0)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Dragonite', 'Inner Focus', ['Extreme Speed'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        // Turn 1: both Fake Outs work. Turn 2: Fake Out is disabled in the requests (compared).
        ['move 1 1, move 1 2', 'move 3, move 4'],
        ['move 2 1, move 2 1', 'move 2 1, move 1 1'],
        // Switching out and back in makes Fake Out usable again.
        ['switch 3, move 3 2', 'move 4, move 4'],
        ['switch 3, move 2 1', 'move 2 1, move 1 2'],
        ['move 1 1, move 2 1', 'move 2 1, move 1 2'],
      ],
    },
    {
      name: 'protect-streak-decay',
      teams: [
        [set('Amoonguss', 'Regenerator', ['Protect', 'Spore', 'Pollen Puff', 'Rage Powder']),
          set('Garchomp', 'Rough Skin', ['Protect', 'Earthquake', 'Dragon Claw', 'Rock Slide'], {evs: sp(20)}),
          set('Gholdengo', 'Good as Gold', ['Shadow Ball']), set('Kingambit', 'Defiant', ['Iron Head'])],
        [set('Garchomp', 'Rough Skin', ['Dragon Claw', 'Earthquake', 'Protect', 'Rock Slide'], {evs: sp(15)}),
          set('Gholdengo', 'Good as Gold', ['Make It Rain', 'Shadow Ball', 'Protect', 'Nasty Plot'], {evs: sp(5)}),
          set('Amoonguss', 'Regenerator', ['Spore']), set('Kingambit', 'Defiant', ['Iron Head'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 1, move 1', 'move 1 2, move 2 1'],
        ['move 1, move 1', 'move 1 2, move 2 1'],
        ['move 1, move 1', 'move 1 2, move 2 1'],
        ['move 1, move 4', 'move 3, move 3'],
        ['move 1, move 1', 'move 1 2, move 2 1'],
      ],
    },
    {
      name: 'encore-and-disable',
      teams: [
        [set('Whimsicott', 'Prankster', ['Encore', 'Tailwind', 'Moonblast', 'Protect'], {evs: sp(20)}),
          set('Gengar', 'Cursed Body', ['Disable', 'Shadow Ball', 'Sludge Bomb', 'Protect'], {evs: sp(25)}),
          set('Garchomp', 'Rough Skin', ['Earthquake']), set('Kingambit', 'Defiant', ['Iron Head'])],
        [set('Dragonite', 'Inner Focus', ['Dragon Dance', 'Extreme Speed', 'Fire Punch', 'Protect'], {evs: sp(5)}),
          set('Metagross', 'Clear Body', ['Iron Defense', 'Meteor Mash', 'Zen Headbutt', 'Protect'], {evs: sp(0)}),
          set('Amoonguss', 'Regenerator', ['Spore']), set('Gholdengo', 'Good as Gold', ['Shadow Ball'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 3 1, move 2 2', 'move 1, move 1'],
        ['move 1 1, move 1 2', 'move 1, move 2 1'],
        ['move 3 2, move 2 1', 'move 1, move 3 1'],
        ['move 3 2, move 2 1', 'move 1, move 3 1'],
        ['move 3 2, move 2 1', 'move 2 1, move 2 2'],
        ['move 3 2, move 2 1', 'move 3 1, move 2 2'],
      ],
    },
    {
      name: 'choice-lock-and-switch',
      teams: [
        [set('Gholdengo', 'Good as Gold', ['Make It Rain', 'Shadow Ball', 'Trick', 'Focus Blast'], {item: 'Choice Scarf', evs: sp(10)}),
          set('Garchomp', 'Rough Skin', ['Earthquake', 'Dragon Claw', 'Rock Slide', 'Swords Dance'], {item: 'Choice Band', evs: sp(15)}),
          set('Kingambit', 'Defiant', ['Iron Head', 'Sucker Punch']), set('Amoonguss', 'Regenerator', ['Spore'])],
        [set('Dragonite', 'Inner Focus', ['Dragon Claw', 'Extreme Speed', 'Fire Punch', 'Protect'], {evs: sp(5)}),
          set('Metagross', 'Clear Body', ['Iron Defense', 'Meteor Mash', 'Zen Headbutt', 'Protect'], {evs: sp(0)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Incineroar', 'Intimidate', ['Fake Out'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 2 1, move 2 2', 'move 1 1, move 2 2'],
        ['move 2 2, move 2 1', 'move 4, move 4'],
        ['switch 3, move 2 1', 'move 1 2, move 2 1'],
        ['move 1 1, move 3', 'move 2 2, move 3 1'],
      ],
    },
    {
      name: 'two-turn-moves',
      teams: [
        [set('Torkoal', 'Drought', ['Solar Beam', 'Heat Wave', 'Protect', 'Body Press'], {evs: sp(0)}),
          set('Venusaur', 'Overgrow', ['Solar Beam', 'Sludge Bomb', 'Protect', 'Sleep Powder'], {evs: sp(10)}),
          set('Garchomp', 'Rough Skin', ['Earthquake']), set('Kingambit', 'Defiant', ['Iron Head'])],
        [set('Dragonite', 'Inner Focus', ['Fly', 'Extreme Speed', 'Fire Punch', 'Protect'], {evs: sp(5)}),
          set('Corviknight', 'Pressure', ['Brave Bird', 'Iron Defense', 'Roost', 'Protect'], {evs: sp(3)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Gholdengo', 'Good as Gold', ['Shadow Ball'])],
      ],
      picks: ['3412', '1234'],
      turns: [
        ['switch 3, switch 4', 'move 1 1, move 2'],
        ['move 1 1, move 1 2', 'move 1 1, move 4'],
        ['move 1 2, move 1 1', 'move 2 1, move 1 2'],
        ['move 1 2, move 1 1', 'move 1 1, move 3'],
      ],
    },
    {
      name: 'double-ko-replacement-with-pass',
      teams: [
        [set('Gholdengo', 'Good as Gold', ['Make It Rain', 'Shadow Ball', 'Protect', 'Nasty Plot'], {evs: sp(10)}),
          set('Garchomp', 'Rough Skin', ['Earthquake', 'Dragon Claw', 'Protect', 'Rock Slide'], {evs: sp(15)}),
          set('Kingambit', 'Defiant', ['Iron Head'])],
        [set('Magikarp', 'Swift Swim', ['Splash'], {evs: sp(0)}), set('Feebas', 'Swift Swim', ['Splash'], {evs: sp(1)}),
          set('Wishiwashi', 'Schooling', ['Splash'], {evs: sp(2)})],
      ],
      picks: ['123', '123'],
      turns: [
        ['move 1, move 4', 'move 1, move 1'],
        ['move 2 1, move 2 1', 'move 1, pass'],
      ],
    },
    {
      name: 'mega-evolution-speed',
      teams: [
        [set('Charizard', 'Blaze', ['Heat Wave', 'Air Slash', 'Protect', 'Solar Beam'], {item: 'Charizardite Y', evs: sp(12)}),
          set('Kingambit', 'Defiant', ['Iron Head', 'Sucker Punch', 'Protect', 'Swords Dance'], {evs: sp(0)}),
          set('Garchomp', 'Rough Skin', ['Earthquake']), set('Amoonguss', 'Regenerator', ['Spore'])],
        [set('Gengar', 'Cursed Body', ['Shadow Ball', 'Sludge Bomb', 'Protect', 'Icy Wind'], {item: 'Gengarite', evs: sp(4)}),
          set('Metagross', 'Clear Body', ['Iron Defense', 'Meteor Mash', 'Zen Headbutt', 'Protect'], {evs: sp(1)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Incineroar', 'Intimidate', ['Fake Out'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 1 mega, move 1 2', 'move 1 1 mega, move 2 1'],
        ['move 2 1, move 1 1', 'move 4, move 3 1'],
      ],
    },
    {
      name: 'trick-room-ordering',
      teams: [
        [set('Hatterene', 'Magic Bounce', ['Trick Room', 'Dazzling Gleam', 'Psychic', 'Protect'], {evs: sp(0)}),
          set('Torkoal', 'Drought', ['Heat Wave', 'Eruption', 'Protect', 'Body Press'], {evs: sp(1)}),
          set('Garchomp', 'Rough Skin', ['Earthquake']), set('Kingambit', 'Defiant', ['Iron Head'])],
        [set('Dragonite', 'Inner Focus', ['Dragon Claw', 'Extreme Speed', 'Fire Punch', 'Protect'], {evs: sp(5)}),
          set('Gholdengo', 'Good as Gold', ['Make It Rain', 'Shadow Ball', 'Protect', 'Nasty Plot'], {evs: sp(8)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Incineroar', 'Intimidate', ['Fake Out'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 1, move 3', 'move 4, move 4'],
        ['move 2, move 1', 'move 1 1, move 2 2'],
        ['move 3 2, move 4', 'move 4, move 4'],
      ],
    },
    {
      name: 'tailwind-duration',
      teams: [
        [set('Whimsicott', 'Prankster', ['Tailwind', 'Moonblast', 'Encore', 'Protect'], {evs: sp(20)}),
          set('Kingambit', 'Defiant', ['Iron Head', 'Sucker Punch', 'Protect', 'Swords Dance'], {evs: sp(0)}),
          set('Garchomp', 'Rough Skin', ['Earthquake']), set('Amoonguss', 'Regenerator', ['Spore'])],
        [set('Corviknight', 'Pressure', ['Tailwind', 'Brave Bird', 'Roost', 'Protect'], {evs: sp(7)}),
          set('Metagross', 'Clear Body', ['Iron Defense', 'Meteor Mash', 'Zen Headbutt', 'Protect'], {evs: sp(2)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Gholdengo', 'Good as Gold', ['Shadow Ball'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 1, move 4', 'move 4, move 2 1'],
        ['move 4, move 1 2', 'move 1, move 1'],
        ['move 4, move 1 2', 'move 4, move 1'],
        ['move 2 1, move 1 2', 'move 3, move 1'],
        ['move 2 1, move 1 2', 'move 3, move 1'],
        ['move 2 1, move 1 2', 'move 3, move 1'],
      ],
    },
    {
      name: 'follow-me-rage-powder',
      teams: [
        [set('Clefairy', 'Friend Guard', ['Follow Me', 'Moonblast', 'Protect', 'Helping Hand'], {item: 'Eviolite', evs: sp(2)}),
          set('Garchomp', 'Rough Skin', ['Earthquake', 'Dragon Claw', 'Protect', 'Rock Slide'], {evs: sp(15)}),
          set('Kingambit', 'Defiant', ['Iron Head']), set('Gholdengo', 'Good as Gold', ['Shadow Ball'])],
        [set('Amoonguss', 'Regenerator', ['Rage Powder', 'Spore', 'Pollen Puff', 'Protect'], {evs: sp(0)}),
          set('Dragonite', 'Inner Focus', ['Dragon Claw', 'Extreme Speed', 'Fire Punch', 'Protect'], {evs: sp(5)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Incineroar', 'Intimidate', ['Fake Out'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 1, move 2 2', 'move 1, move 1 2'],
        ['move 2 2, move 2 2', 'move 1, move 1 2'],
        ['move 1, move 2 1', 'move 4, move 1 2'],
      ],
    },
    {
      name: 'wide-guard-quick-guard',
      teams: [
        [set('Hitmontop', 'Intimidate', ['Wide Guard', 'Quick Guard', 'Fake Out', 'Close Combat'], {evs: sp(5)}),
          set('Gholdengo', 'Good as Gold', ['Make It Rain', 'Shadow Ball', 'Protect', 'Nasty Plot'], {evs: sp(10)}),
          set('Kingambit', 'Defiant', ['Iron Head']), set('Garchomp', 'Rough Skin', ['Earthquake'])],
        [set('Garchomp', 'Rough Skin', ['Rock Slide', 'Earthquake', 'Dragon Claw', 'Protect'], {evs: sp(15)}),
          set('Dragonite', 'Inner Focus', ['Extreme Speed', 'Dragon Claw', 'Fire Punch', 'Protect'], {evs: sp(3)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Incineroar', 'Intimidate', ['Fake Out'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 1, move 2 1', 'move 1, move 1 2'],
        ['move 2, move 2 1', 'move 2, move 1 2'],
        ['move 4 1, move 1', 'move 1, move 1 1'],
      ],
    },
    {
      name: 'intimidate-parting-shot-eject-button',
      teams: [
        [set('Incineroar', 'Intimidate', ['Parting Shot', 'Fake Out', 'Flare Blitz', 'Knock Off'], {evs: sp(3)}),
          set('Kingambit', 'Defiant', ['Iron Head', 'Sucker Punch', 'Protect', 'Swords Dance'], {evs: sp(0)}),
          set('Gyarados', 'Intimidate', ['Waterfall', 'Dragon Dance']), set('Garchomp', 'Rough Skin', ['Earthquake'])],
        [set('Dragonite', 'Inner Focus', ['Dragon Claw', 'Extreme Speed', 'Fire Punch', 'Protect'], {item: 'Eject Button', evs: sp(6)}),
          set('Gholdengo', 'Good as Gold', ['Make It Rain', 'Shadow Ball', 'Protect', 'Nasty Plot'], {evs: sp(9)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Corviknight', 'Mirror Armor', ['Brave Bird'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 1 1, move 1 1', 'move 4, move 4'],
        ['move 3 1, move 2 1', 'move 2 1, move 2 1'],
        ['move 1 1, move 1 2', 'move 1 1, move 1 2'],
      ],
    },
    {
      name: 'spread-move-damage',
      teams: [
        [set('Garchomp', 'Rough Skin', ['Earthquake', 'Rock Slide', 'Dragon Claw', 'Protect'], {evs: sp(15)}),
          set('Gholdengo', 'Good as Gold', ['Make It Rain', 'Shadow Ball', 'Protect', 'Nasty Plot'], {evs: sp(10)}),
          set('Kingambit', 'Defiant', ['Iron Head']), set('Amoonguss', 'Regenerator', ['Spore'])],
        [set('Dragonite', 'Multiscale', ['Dragon Claw', 'Extreme Speed', 'Fire Punch', 'Protect'], {evs: sp(5)}),
          set('Incineroar', 'Intimidate', ['Fake Out', 'Flare Blitz', 'Protect', 'Parting Shot'], {evs: sp(3)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Metagross', 'Clear Body', ['Meteor Mash'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 2, move 1', 'move 4, move 3'],
        ['move 1, move 1', 'move 1 2, move 2 1'],
      ],
    },
    {
      name: 'ally-switch',
      teams: [
        [set('Hatterene', 'Magic Bounce', ['Ally Switch', 'Dazzling Gleam', 'Psychic', 'Protect'], {evs: sp(0)}),
          set('Garchomp', 'Rough Skin', ['Earthquake', 'Dragon Claw', 'Protect', 'Rock Slide'], {evs: sp(15)}),
          set('Kingambit', 'Defiant', ['Iron Head']), set('Amoonguss', 'Regenerator', ['Spore'])],
        [set('Dragonite', 'Inner Focus', ['Dragon Claw', 'Extreme Speed', 'Fire Punch', 'Protect'], {evs: sp(5)}),
          set('Gholdengo', 'Good as Gold', ['Make It Rain', 'Shadow Ball', 'Protect', 'Nasty Plot'], {evs: sp(8)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Incineroar', 'Intimidate', ['Fake Out'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 1, move 2 1', 'move 1 2, move 2 1'],
        ['move 1, move 2 1', 'move 1 1, move 2 2'],
        ['move 2, move 3', 'move 1 2, move 2 2'],
      ],
    },
    {
      name: 'sleep-and-freeze-turns',
      seed: 7,
      teams: [
        [set('Amoonguss', 'Regenerator', ['Spore', 'Pollen Puff', 'Protect', 'Rage Powder'], {evs: sp(0)}),
          set('Cloyster', 'Skill Link', ['Freeze-Dry', 'Ice Beam', 'Icicle Spear', 'Protect'], {evs: sp(12)}),
          set('Kingambit', 'Defiant', ['Iron Head']), set('Garchomp', 'Rough Skin', ['Earthquake'])],
        [set('Snorlax', 'Thick Fat', ['Body Slam', 'Protect', 'Curse', 'Rest'], {evs: sp(1)}),
          set('Dragonite', 'Inner Focus', ['Dragon Claw', 'Extreme Speed', 'Fire Punch', 'Protect'], {evs: sp(5)}),
          set('Pelipper', 'Drizzle', ['Hurricane']), set('Incineroar', 'Intimidate', ['Fake Out'])],
      ],
      picks: ['1234', '1234'],
      turns: [
        ['move 1 1, move 2 2', 'move 1 1, move 1 1'],
        ['move 2 2, move 2 2', 'move 1 1, move 1 1'],
        ['move 2 1, move 2 2', 'move 1 1, move 1 1'],
        ['move 2 1, move 2 2', 'move 1 1, move 1 1'],
        ['move 2 1, move 2 2', 'move 1 1, move 1 1'],
        ['move 2 1, move 2 2', 'move 1 1, move 1 1'],
      ],
    },
  ];
}

module.exports = {scenarios, set};
