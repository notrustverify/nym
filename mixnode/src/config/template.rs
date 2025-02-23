// Copyright 2020 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

// While using normal toml marshalling would have been way simpler with less overhead,
// I think it's useful to have comments attached to the saved config file to explain behaviour of
// particular fields.
// Note: any changes to the template must be reflected in the appropriate structs.
pub(crate) const CONFIG_TEMPLATE: &str = r#"
# This is a TOML config file.
# For more information, see https://github.com/toml-lang/toml

##### main base mixnode config options #####

[mixnode]
# Version of the mixnode for which this configuration was created.
version = '{{ mixnode.version }}'
    
# Human readable ID of this particular mixnode.
id = '{{ mixnode.id }}'

# Socket address to which this mixnode will bind to and will be listening for packets.
listening_address = '{{ mixnode.listening_address }}'

# Port used for listening for all mixnet traffic.
# (default: 1789)
mix_port = {{ mixnode.mix_port }}

# Port used for listening for verloc traffic.
# (default: 1790)
verloc_port = {{ mixnode.verloc_port }}

# Port used for listening for http requests.
# (default: 8000)
http_api_port = {{ mixnode.http_api_port }}

# Addresses to APIs running on validator from which the node gets the view of the network.
nym_api_urls = [
    {{#each mixnode.nym_api_urls }}
        '{{this}}',
    {{/each}}
]

[storage_paths] 

# Path to file containing private identity key.
keys.private_identity_key_file = '{{ storage_paths.keys.private_identity_key_file }}'

# Path to file containing public identity key.
keys.public_identity_key_file = '{{ storage_paths.keys.public_identity_key_file }}'

# Path to file containing private identity key.
keys.private_sphinx_key_file = '{{ storage_paths.keys.private_sphinx_key_file }}'

# Path to file containing public sphinx key.
keys.public_sphinx_key_file = '{{ storage_paths.keys.public_sphinx_key_file }}'

# Path to file containing description of this node.
node_description = '{{ storage_paths.node_description }}'

##### logging configuration options #####

[logging]

# TODO

"#;
