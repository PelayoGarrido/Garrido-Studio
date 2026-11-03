use crate::{
    Error, Result,
    model::{MAX_TRACKS, Project, finite},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub from: usize,
    pub to: Option<usize>,
    pub gain: f32,
    pub invert: bool,
    pub swap: bool,
}

pub struct Graph {
    pub order: Vec<usize>,
    pub outputs: [Vec<Route>; MAX_TRACKS],
    pub audible: [bool; MAX_TRACKS],
}

impl Graph {
    pub fn compile(project: &Project) -> Result<Self> {
        let count = project.tracks.len();
        if count > MAX_TRACKS || project.routes.len() > 256 {
            return Err(Error::Limit("Demasiadas rutas"));
        }
        let mut outputs: [Vec<Route>; MAX_TRACKS] = std::array::from_fn(|_| Vec::new());
        let mut incoming = [0usize; MAX_TRACKS];
        for r in &project.routes {
            finite(r.gain, 0., 2.)?;
            if r.from >= count || r.to.is_some_and(|to| to >= count || to == r.from) {
                return Err(Error::Invalid("Destino de mezcla no válido"));
            }
            if outputs[r.from].iter().any(|e| e.to == r.to) {
                return Err(Error::Invalid("Ruta duplicada"));
            }
            if let Some(to) = r.to {
                incoming[to] += 1;
            }
            outputs[r.from].push(r.clone());
        }
        for (i, output) in outputs.iter_mut().enumerate().take(count) {
            if output.is_empty() {
                output.push(Route {
                    from: i,
                    to: None,
                    gain: 1.,
                    invert: false,
                    swap: false,
                });
            }
            if output.iter().map(|r| r.gain).sum::<f32>() > 4. {
                return Err(Error::Limit("Ganancia total de envío demasiado alta"));
            }
        }
        let mut order = Vec::with_capacity(count);
        for (i, &n) in incoming.iter().enumerate().take(count) {
            if n == 0 {
                order.push(i);
            }
        }
        let mut cursor = 0;
        while cursor < order.len() {
            let from = order[cursor];
            cursor += 1;
            for r in &outputs[from] {
                if let Some(to) = r.to {
                    incoming[to] -= 1;
                    if incoming[to] == 0 {
                        order.push(to);
                    }
                }
            }
        }
        if order.len() != count {
            return Err(Error::Invalid(
                "La mezcla contiene un bucle de realimentación",
            ));
        }
        let solo = project.tracks.iter().any(|t| t.solo);
        let mut audible = std::array::from_fn(|i| i < count && (!solo || project.tracks[i].solo));
        if solo {
            for &from in &order {
                if audible[from] {
                    for r in &outputs[from] {
                        if let Some(to) = r.to {
                            audible[to] = true;
                        }
                    }
                }
            }
        }
        Ok(Self {
            order,
            outputs,
            audible,
        })
    }
}
