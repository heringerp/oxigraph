#!/usr/bin/env bash

DATASET_SIZE=100000 # number of products in the dataset. There is around 350 triples generated by product.
PARALLELISM=16
cd bsbm-tools
./generate -fc -pc ${DATASET_SIZE} -s nt -fn "explore-${DATASET_SIZE}" -ud -ufn "explore-update-${DATASET_SIZE}"
cargo build --release --manifest-path="../../server/Cargo.toml"
VERSION=$(./../../target/release/oxigraph_server --version | sed 's/oxigraph_server //g')
./../../target/release/oxigraph_server --location oxigraph_data load --file "explore-${DATASET_SIZE}.nt"
./../../target/release/oxigraph_server --location oxigraph_data serve --bind 127.0.0.1:7878 &
sleep 1
./testdriver -mt ${PARALLELISM} -ucf usecases/explore/sparql.txt -o "../bsbm.explore.oxigraph.${VERSION}.${DATASET_SIZE}.${PARALLELISM}.xml" http://127.0.0.1:7878/query
./testdriver -mt ${PARALLELISM} -ucf usecases/exploreAndUpdate/sparql.txt -o "../bsbm.exploreAndUpdate.oxigraph.${VERSION}.${DATASET_SIZE}.${PARALLELISM}.xml" http://127.0.0.1:7878/query -u http://127.0.0.1:7878/update -udataset "explore-update-${DATASET_SIZE}.nt"
#./testdriver -mt ${PARALLELISM} -ucf usecases/businessIntelligence/sparql.txt -o "../bsbm.businessIntelligence.${VERSION}.${DATASET_SIZE}.${PARALLELISM}.xml" "http://127.0.0.1:7878/query"
kill $!
rm -r oxigraph_data
rm "explore-${DATASET_SIZE}.nt"
rm "explore-update-${DATASET_SIZE}.nt"
rm -r td_data
