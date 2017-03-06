use std::ops::Deref;

use bitcoinrpc;
use bitcoin::util::base58::{FromBase58, ToBase58};
use bitcoin::network::constants::Network;

use exonum::crypto::HexValue;

use transactions::{BitcoinTx, TxKind};
use SATOSHI_DIVISOR;
use multisig::RedeemScript;
use btc;
use config::AnchoringRpcConfig;

#[cfg(not(feature="sandbox_tests"))]
pub use bitcoinrpc::Client as RpcClient;
#[cfg(feature="sandbox_tests")]
pub use sandbox::SandboxClient as RpcClient;

pub type Result<T> = bitcoinrpc::Result<T>;
pub type Error = bitcoinrpc::Error;

#[derive(Debug)]
pub struct AnchoringRpc(pub RpcClient);

impl AnchoringRpc {
    pub fn new(cfg: AnchoringRpcConfig) -> AnchoringRpc {
        AnchoringRpc(RpcClient::new(cfg.host, cfg.username, cfg.password))
    }

    pub fn config(&self) -> AnchoringRpcConfig {
        AnchoringRpcConfig {
            host: self.0.url().to_string(),
            username: self.0.username().clone(),
            password: self.0.password().clone(),
        }
    }

    pub fn gen_keypair(&self, account: &str) -> Result<(btc::PublicKey, btc::PrivateKey)> {
        let addr = self.0.getnewaddress(account)?;
        let info = self.0.validateaddress(&addr)?;
        let privkey = self.0.dumpprivkey(&addr)?;

        let pubkey = btc::PublicKey::from_hex(info.pubkey).unwrap();
        let privkey = btc::PrivateKey::from_base58check(&privkey).unwrap();
        Ok((pubkey, privkey))
    }

    pub fn get_transaction(&self, txid: &str) -> Result<BitcoinTx> {
        let tx = self.0.getrawtransaction(txid)?;
        Ok(BitcoinTx::from_hex(tx).unwrap())
    }

    pub fn get_transaction_info(&self, txid: &str) -> Result<bitcoinrpc::RawTransactionInfo> {
        self.0.getrawtransaction_verbose(txid)
    }

    pub fn send_transaction(&self, tx: BitcoinTx) -> Result<()> {
        let tx_hex = tx.to_hex();
        self.0.sendrawtransaction(&tx_hex)?;
        Ok(())
    }

    pub fn send_to_address(&self, address: &btc::Address, funds: u64) -> Result<BitcoinTx> {
        let addr = address.to_base58check();
        let funds_str = (funds as f64 / SATOSHI_DIVISOR).to_string();
        let utxo_txid = self.0.sendtoaddress(&addr, &funds_str)?;
        Ok(self.get_transaction(&utxo_txid)?)
    }

    pub fn create_multisig_address<'a, I>(&self,
                                          network: Network,
                                          count: u8,
                                          pub_keys: I)
                                          -> Result<(RedeemScript, btc::Address)>
        where I: IntoIterator<Item = &'a btc::PublicKey>
    {
        let redeem_script = RedeemScript::from_pubkeys(pub_keys, count).compressed(network);
        let addr = btc::Address::from_script(&redeem_script, network);

        self.0.importaddress(&addr.to_base58check(), "multisig", false, false)?;
        Ok((redeem_script, addr))
    }

    pub fn get_last_anchoring_transactions(&self,
                                           addr: &str,
                                           limit: u32)
                                           -> Result<Vec<bitcoinrpc::TransactionInfo>> {
        self.0
            .listtransactions(limit, 0, true)
            .map(|v| {
                v.into_iter()
                    .rev()
                    .filter(|tx| tx.address == Some(addr.into()))
                    .collect::<Vec<_>>()
            })
    }

    pub fn get_unspent_transactions(&self,
                                    min_conf: u32,
                                    max_conf: u32,
                                    addr: &str)
                                    -> Result<Vec<bitcoinrpc::UnspentTransactionInfo>> {
        self.0.listunspent(min_conf, max_conf, [addr])
    }

    pub fn unspent_transactions(&self, addr: &btc::Address) -> Result<Vec<BitcoinTx>> {
        let unspent_txs = self.get_unspent_transactions(0, 9999999, &addr.to_base58check())?;
        // FIXME Develop searching algorhytm
        let mut txs = Vec::new();
        for info in unspent_txs {
            let raw_tx = self.get_transaction(&info.txid)?;
            match TxKind::from(raw_tx) {
                TxKind::Anchoring(tx) => txs.push(tx.into()),
                TxKind::FundingTx(tx) => txs.push(tx.into()),
                TxKind::Other(_) => {}
            }
        }
        Ok(txs)
    }
}

impl Deref for AnchoringRpc {
    type Target = RpcClient;

    fn deref(&self) -> &RpcClient {
        &self.0
    }
}