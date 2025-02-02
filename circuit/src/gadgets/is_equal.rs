use halo2wrong::halo2::{
	arithmetic::FieldExt,
	circuit::{AssignedCell, Layouter, Region},
	plonk::{Advice, Column, ConstraintSystem, Error, Selector},
	poly::Rotation,
};

use super::is_zero::{IsZeroChip, IsZeroConfig};

#[derive(Clone, Debug)]
/// Configuration elements for the circuit are defined here.
pub struct IsEqualConfig {
	/// Constructs is_zero circuit elements.
	is_zero: IsZeroConfig,
	/// Configures a column for the lhs.
	lhs: Column<Advice>,
	/// Configures a column for the rhs.
	rhs: Column<Advice>,
	/// Configures a column for the out.
	out: Column<Advice>,
	/// Configures a fixed boolean value for each row of the circuit.
	s: Selector,
}

#[derive(Clone)]
/// Constructs individual cells for the configuration elements.
pub struct IsEqualChip<F: FieldExt> {
	/// Assigns a cell for the lhs.
	lhs: AssignedCell<F, F>,
	/// Assigns a cell for the rhs.
	rhs: AssignedCell<F, F>,
}

impl<F: FieldExt> IsEqualChip<F> {
	/// Create a new chip.
	pub fn new(x: AssignedCell<F, F>, y: AssignedCell<F, F>) -> Self {
		Self { lhs: x, rhs: y }
	}

	/// Make the circuit config.
	pub fn configure(meta: &mut ConstraintSystem<F>) -> IsEqualConfig {
		let is_zero_config = IsZeroChip::configure(meta);
		let lhs = meta.advice_column();
		let rhs = meta.advice_column();
		let out = meta.advice_column();
		let s = meta.selector();

		meta.enable_equality(lhs);
		meta.enable_equality(rhs);
		meta.enable_equality(out);

		meta.create_gate("is_equal", |v_cells| {
			let lhs_exp = v_cells.query_advice(lhs, Rotation::cur());
			let rhs_exp = v_cells.query_advice(rhs, Rotation::cur());
			let out_exp = v_cells.query_advice(out, Rotation::cur());
			let s_exp = v_cells.query_selector(s);
			vec![
				// (x + y) - z == 0
				// Example:
				// let y = 123;
				// let z = 123;
				// let x = (y - z);
				// x;
				//
				// x = (123 - 123) = 0 => Checking the constraint (0 + 123) - 123 == 0
				s_exp * ((out_exp + rhs_exp) - lhs_exp),
			]
		});

		IsEqualConfig { is_zero: is_zero_config, lhs, rhs, out, s }
	}

	/// Synthesize the circuit.
	pub fn synthesize(
		&self, config: IsEqualConfig, mut layouter: impl Layouter<F>,
	) -> Result<AssignedCell<F, F>, Error> {
		let out = layouter.assign_region(
			|| "temp",
			|mut region: Region<'_, F>| {
				config.s.enable(&mut region, 0)?;
				let assigned_lhs = self.lhs.copy_advice(|| "lhs", &mut region, config.lhs, 0)?;
				let assigned_rhs = self.rhs.copy_advice(|| "rhs", &mut region, config.rhs, 0)?;

				let out = assigned_lhs.value().cloned() - assigned_rhs.value();

				let assigned_out = region.assign_advice(|| "lhs", config.out, 0, || out)?;
				Ok(assigned_out)
			},
		)?;

		// If variable out holds 0 as value, that means is_zero circuit will return 1.
		let is_zero_chip = IsZeroChip::new(out);
		let is_zero = is_zero_chip.synthesize(config.is_zero, layouter.namespace(|| "is_zero"))?;
		Ok(is_zero)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::utils::{generate_params, prove_and_verify};
	use halo2wrong::{
		curves::bn256::{Bn256, Fr},
		halo2::{
			circuit::{SimpleFloorPlanner, Value},
			dev::MockProver,
			plonk::{Circuit, Instance},
		},
	};

	#[derive(Clone)]
	struct TestConfig {
		is_zero: IsEqualConfig,
		pub_ins: Column<Instance>,
		temp: Column<Advice>,
	}

	#[derive(Clone)]
	struct TestCircuit<F: FieldExt> {
		x: F,
		y: F,
	}

	impl<F: FieldExt> TestCircuit<F> {
		fn new(x: F, y: F) -> Self {
			Self { x, y }
		}
	}

	impl<F: FieldExt> Circuit<F> for TestCircuit<F> {
		type Config = TestConfig;
		type FloorPlanner = SimpleFloorPlanner;

		fn without_witnesses(&self) -> Self {
			self.clone()
		}

		fn configure(meta: &mut ConstraintSystem<F>) -> TestConfig {
			let is_zero = IsEqualChip::configure(meta);
			let temp = meta.advice_column();
			let instance = meta.instance_column();

			meta.enable_equality(instance);
			meta.enable_equality(temp);

			TestConfig { is_zero, pub_ins: instance, temp }
		}

		fn synthesize(
			&self, config: TestConfig, mut layouter: impl Layouter<F>,
		) -> Result<(), Error> {
			let (lhs, rhs) = layouter.assign_region(
				|| "temp",
				|mut region: Region<'_, F>| {
					let lhs = region.assign_advice(
						|| "temp_x",
						config.temp,
						0,
						|| Value::known(self.x),
					)?;
					let rhs = region.assign_advice(
						|| "temp_y",
						config.temp,
						1,
						|| Value::known(self.y),
					)?;

					Ok((lhs, rhs))
				},
			)?;
			let is_eq_chip = IsEqualChip::new(lhs, rhs);
			let is_eq = is_eq_chip.synthesize(config.is_zero, layouter.namespace(|| "is_zero"))?;
			layouter.constrain_instance(is_eq.cell(), config.pub_ins, 0)?;
			Ok(())
		}
	}

	#[test]
	fn test_is_equal() {
		// Testing equal values.
		let test_chip = TestCircuit::new(Fr::from(123), Fr::from(123));

		let pub_ins = vec![Fr::one()];
		let k = 4;
		let prover = MockProver::run(k, &test_chip, vec![pub_ins]).unwrap();
		assert_eq!(prover.verify(), Ok(()));
	}

	#[test]
	fn test_is_not_equal() {
		// Testing not equal values.
		let test_chip = TestCircuit::new(Fr::from(123), Fr::from(124));

		let pub_ins = vec![Fr::zero()];
		let k = 4;
		let prover = MockProver::run(k, &test_chip, vec![pub_ins]).unwrap();
		assert_eq!(prover.verify(), Ok(()));
	}

	#[test]
	fn test_is_equal_production() {
		let test_chip = TestCircuit::new(Fr::from(123), Fr::from(123));

		let k = 4;
		let rng = &mut rand::thread_rng();
		let params = generate_params(k);
		let res = prove_and_verify::<Bn256, _, _>(params, test_chip, &[&[Fr::one()]], rng).unwrap();

		assert!(res);
	}
}
