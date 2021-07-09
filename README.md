# Rental agreement

Manage rental agreements between a property owner and tenant.


# Instructions
## Building
```
cargo build-bpf
```

## Deploy to devnet
### Validate Solana Configuration
Your solana environment should point to testnet or devnet

```
solana config set --url https://api.testnet.solana.com
```

verify it

```
solana config get
===>

Config File: /home/alex/.config/solana/cli/config.yml
RPC URL: https://api.testnet.solana.com 
WebSocket URL: wss://api.testnet.solana.com/ (computed)
Keypair Path: /home/alex/.config/solana/id.json 
Commitment: confirmed 
```

### Deploy BPF Program

add balance
```
solana airdrop 1
# do it several times
```

then

```
solana program deploy ./target/deploy/rental_agreement.so
```