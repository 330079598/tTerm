#!/bin/bash
seq 1 12000
sleep 8
for i in $(seq 10 70); do seq $((i*10000)) $((i*10000+9999)); sleep 0.1; done
sleep 120
