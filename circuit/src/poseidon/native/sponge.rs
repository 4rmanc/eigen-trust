use crate::poseidon::{native::Poseidon, RoundParams};
use halo2wrong::halo2::arithmetic::FieldExt;
use std::marker::PhantomData;

/// Constructs objects.
pub struct PoseidonSponge<F: FieldExt, const WIDTH: usize, P>
where
	P: RoundParams<F, WIDTH>,
{
	/// Constructs a vector for the inputs.
	inputs: Vec<F>,
	/// Constructs a phantom data for the parameters.
	_params: PhantomData<P>,
}

impl<F: FieldExt, const WIDTH: usize, P> PoseidonSponge<F, WIDTH, P>
where
	P: RoundParams<F, WIDTH>,
{
	/// Create objects.
	pub fn new() -> Self {
		Self { inputs: Vec::new(), _params: PhantomData }
	}

	/// Clones and appends all elements from a slice to the vec.
	pub fn update(&mut self, inputs: &[F]) {
		self.inputs.extend_from_slice(inputs);
	}

	/// Absorb the data in and split it into
	/// chunks of size WIDTH.
	pub fn load_state(chunk: &[F]) -> [F; WIDTH] {
		assert!(chunk.len() <= WIDTH);
		let mut fixed_chunk = [F::zero(); WIDTH];
		fixed_chunk[..chunk.len()].copy_from_slice(chunk);
		fixed_chunk
	}

	/// Squeeze the data out by
	/// permuting until no more chunks are left.
	pub fn squeeze(&mut self) -> F {
		assert!(!self.inputs.is_empty());

		let mut state = [F::zero(); WIDTH];

		for chunk in self.inputs.chunks(WIDTH) {
			let loaded_state = Self::load_state(chunk);
			let input = loaded_state.zip(state).map(|(lhs, rhs)| lhs + rhs);

			let pos = Poseidon::<_, WIDTH, P>::new(input);
			state = pos.permute();
		}

		state[0]
	}
}
