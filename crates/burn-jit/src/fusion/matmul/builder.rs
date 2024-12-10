use crate::fusion::on_write::ir::Arg;
use burn_fusion::{OptimizationBuilder, OptimizationStatus};
use burn_tensor::repr::{FloatOperationDescription, OperationDescription};

use crate::{
    fusion::{
        on_write::{builder::FuseOnWriteBuilder, ir::ElemwisePrecision},
        JitOptimization,
    },
    JitRuntime,
};

use super::optimization::MatmulOptimization;

/// Fused element wise operations that are normally memory bound.
pub(crate) struct MatmulBuilder<R: JitRuntime> {
    builder: FuseOnWriteBuilder,
    device: R::Device,
    matmul_op: Option<(Arg, Arg, Arg)>,
}

impl<R: JitRuntime> MatmulBuilder<R> {
    pub fn new(device: R::Device, bool_precision: ElemwisePrecision) -> Self {
        let client = R::client(&device);
        let props = client.properties();
        let max_bindings = props.hardware_properties().max_bindings;

        Self {
            builder: FuseOnWriteBuilder::new(max_bindings, bool_precision),
            device,
            matmul_op: None,
        }
    }
}

impl<R: JitRuntime> OptimizationBuilder<JitOptimization<R>> for MatmulBuilder<R> {
    fn register(&mut self, operation: &OperationDescription) {
        if let OptimizationStatus::Closed = self.builder.status() {
            return;
        }

        if self.matmul_op.is_none() {
            if let OperationDescription::Float(_, FloatOperationDescription::Matmul(op)) = operation
            {
                let lhs = self.builder.input_unhandled(&op.lhs);
                let rhs = self.builder.input_unhandled(&op.rhs);
                let out = self.builder.output_unhandled(&op.out);

                self.matmul_op = Some((lhs, rhs, out));
            } else {
                self.builder.close();
            }
        } else {
            self.builder.register(operation)
        }
    }

    fn build(&self) -> JitOptimization<R> {
        let client = R::client(&self.device);
        let trace = self.builder.build();

        let matmul = MatmulOptimization::<R>::new(
            trace,
            client,
            self.device.clone(),
            self.len(),
            self.matmul_op.unwrap(),
        );
        println!("BUILD  {:?}", matmul.to_state());

        JitOptimization::Matmul(matmul)
    }

    fn reset(&mut self) {
        self.builder.reset();
        self.matmul_op = None;
    }

    fn status(&self) -> burn_fusion::OptimizationStatus {
        self.builder.status()
    }

    fn properties(&self) -> burn_fusion::OptimizationProperties {
        let mut properties = self.builder.properties();
        properties.score += 1;
        properties
    }

    fn len(&self) -> usize {
        self.builder.len() + 1 // Matmul operation isn't registered in the builder
    }
}
