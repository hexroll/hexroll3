/*
// Copyright (C) 2020-2026 Pen, Dice & Paper
//
// This program is dual-licensed under the following terms:
//
// Option 1: GNU Affero General Public License (AGPL)
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <http://www.gnu.org/licenses/>.
//
// Option 2: Commercial License
// For commercial use, you are required to obtain a separate commercial
// license. Please contact ithai at pendicepaper.com
// for more information about commercial licensing terms.
*/

//
// let temp: Vec<lyon::math::Point> = polygon_points
//     .iter()
//     .map(|v| lyon::math::Point::new(v.x, v.y))
//     .collect();
// commands.spawn((
//     Mesh3d(meshes.add(make_mesh_from_outline(&temp))),
//     MeshMaterial3d(materials.add(StandardMaterial {
//         unlit: true,
//         base_color: Color::srgba_u8(
//             rand::random::<u8>(),
//             rand::random::<u8>(),
//             rand::random::<u8>(),
//             130,
//         ),
//         alpha_mode: AlphaMode::Blend,
//         ..default()
//     })),
//     Transform::from_xyz(0.0, 200.0 + count, 0.0),
// ));
