/// Implementation of Edwards on Bn254 curve AKA BabyJubJub
pub mod ed_on_bn254;
/// Helper functions for point arithmetic
pub mod ops;

use crate::{params::poseidon_bn254_5x5::Params, poseidon::native::Poseidon, utils::to_wide};
use ed_on_bn254::{Point, B8, SUBORDER};
use halo2wrong::{
	curves::{bn256::Fr, FieldExt},
	halo2::arithmetic::Field,
};
use num_bigint::BigUint;
use rand::RngCore;

type Hasher = Poseidon<Fr, 5, Params>;

/// Hashes the input with using the BLAKE hash function.
fn blh(b: &[u8]) -> Vec<u8> {
	let mut hash = [0; 64];
	blake::hash(512, b, &mut hash).unwrap();
	hash.to_vec()
}

/// Configures a structure for the secret key.
pub struct SecretKey(BigUint, Fr);

impl SecretKey {
	/// Randomly generates a field element and returns
	/// two hashed values from it.
	pub fn random<R: RngCore + Clone>(rng: &mut R) -> Self {
		let a = Fr::random(rng);
		let hash: Vec<u8> = blh(&a.to_bytes());
		let sk0 = BigUint::from_bytes_le(&hash[..32]);

		let bytes_wide = to_wide(&hash[32..]);
		let sk1 = Fr::from_bytes_wide(&bytes_wide);
		SecretKey(sk0, sk1)
	}

	/// Returns a public key from the secret key.
	pub fn public(&self) -> PublicKey {
		let a = B8.mul_scalar(&self.0.to_bytes_le());
		PublicKey(a.affine())
	}
}

/// Configures a structure for the public key.
pub struct PublicKey(pub Point);

#[derive(Clone)]
/// Configures signature objects.
pub struct Signature {
	/// Constructs a point for the R.
	pub big_r: Point,
	/// Constructs a field element for the s.
	pub s: Fr,
}

/// Returns a signature from given keys and message.
pub fn sign(sk: &SecretKey, pk: &PublicKey, m: Fr) -> Signature {
	let inputs = [Fr::zero(), sk.1, m, Fr::zero(), Fr::zero()];
	let r = Hasher::new(inputs).permute()[0];
	let r_bn = BigUint::from_bytes_le(&r.to_bytes());

	// R = B8 * r
	let big_r = B8.mul_scalar(&r.to_bytes()).affine();
	// H(R || PK || M)
	let m_hash_input = [big_r.x, big_r.y, pk.0.x, pk.0.y, m];
	let m_hash = Hasher::new(m_hash_input).permute()[0];
	let m_hash_bn = BigUint::from_bytes_le(&m_hash.to_bytes());
	// S = r + H(R || PK || M) * sk0   (mod n)
	let s = r_bn + &sk.0 * m_hash_bn;
	let s = s % BigUint::from_bytes_le(&SUBORDER.to_bytes());
	let s = Fr::from_bytes_wide(&to_wide(&s.to_bytes_le()));

	Signature { big_r, s }
}

/// Checks if the signature holds with the given PK and message.
pub fn verify(sig: &Signature, pk: &PublicKey, m: Fr) -> bool {
	if sig.s > SUBORDER {
		// S can't be higher than SUBORDER
		return false;
	}
	// Cl = s * G
	let cl = B8.mul_scalar(&sig.s.to_bytes());
	// H(R || PK || M)
	let m_hash_input = [sig.big_r.x, sig.big_r.y, pk.0.x, pk.0.y, m];
	let m_hash = Hasher::new(m_hash_input).permute()[0];
	let pk_h = pk.0.mul_scalar(&m_hash.to_bytes());
	// Cr = R + H(R || PK || M) * PK
	let cr = sig.big_r.projective().add(&pk_h);
	cr.affine().equals(cl.affine())
}

#[cfg(test)]
mod test {
	use super::*;
	use halo2wrong::curves::group::ff::PrimeField;
	use rand::thread_rng;

	#[test]
	fn should_sign_and_verify() {
		// Testing a valid case.
		let mut rng = thread_rng();

		let sk = SecretKey::random(&mut rng);
		let pk = sk.public();

		let m = Fr::from_str_vartime("123456789012345678901234567890").unwrap();
		let sig = sign(&sk, &pk, m);
		let res = verify(&sig, &pk, m);

		assert!(res);
	}

	#[test]
	fn test_invalid_big_r() {
		// Testing invalid R.
		let mut rng = thread_rng();

		let sk = SecretKey::random(&mut rng);
		let pk = sk.public();

		let inputs = [Fr::zero(), Fr::one(), Fr::one(), Fr::zero(), Fr::zero()];
		let different_r = Hasher::new(inputs).permute()[0];

		let m = Fr::from_str_vartime("123456789012345678901234567890").unwrap();
		let mut sig = sign(&sk, &pk, m);

		sig.big_r = B8.mul_scalar(&different_r.to_bytes()).affine();
		let res = verify(&sig, &pk, m);

		assert_eq!(res, false);
	}

	#[test]
	fn test_invalid_s() {
		// Testing invalid s.
		let mut rng = thread_rng();

		let sk = SecretKey::random(&mut rng);
		let pk = sk.public();

		let m = Fr::from_str_vartime("123456789012345678901234567890").unwrap();
		let mut sig = sign(&sk, &pk, m);
		sig.s = sig.s.add(&Fr::from(1));
		let res = verify(&sig, &pk, m);

		assert_eq!(res, false);
	}

	#[test]
	fn test_invalid_pk() {
		// Testing invalid public key.
		let mut rng = thread_rng();

		let sk1 = SecretKey::random(&mut rng);
		let pk1 = sk1.public();

		let sk2 = SecretKey::random(&mut rng);
		let pk2 = sk2.public();

		let m = Fr::from_str_vartime("123456789012345678901234567890").unwrap();
		let sig = sign(&sk1, &pk1, m);
		let res = verify(&sig, &pk2, m);

		assert_eq!(res, false);
	}

	#[test]
	fn test_invalid_message() {
		// Testing invalid message.
		let mut rng = thread_rng();

		let sk = SecretKey::random(&mut rng);
		let pk = sk.public();

		let m1 = Fr::from_str_vartime("123456789012345678901234567890").unwrap();
		let sig = sign(&sk, &pk, m1);
		let m2 = Fr::from_str_vartime("123456789012345678901234567890123123").unwrap();
		let res = verify(&sig, &pk, m2);

		assert_eq!(res, false);
	}
}
