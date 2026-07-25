// Pseudocode: illustrates intended API shape, not a compiling interface.

use asupersync::{Cx, Outcome};
use continuum::{
    effect, invariant, model, observer, refinement, system, DomainPack,
};

#[model]
mod abstract_register {
    pub struct State {
        pub chosen: Map<Epoch, Value>,
    }

    #[action]
    pub fn choose(s: &State, epoch: Epoch, value: Value) -> Relation<State> {
        require!(s.chosen.get(&epoch).is_none_or(|v| v == &value));
        next!(s, chosen[epoch] = value)
    }

    #[invariant]
    pub fn agreement(s: &State) -> bool {
        // A map representation makes uniqueness structural; an independent
        // witness property checks the quorum relation.
        true
    }
}

#[system(runtime = "asupersync")]
mod replicated_register {
    pub struct Replica {
        epoch: Epoch,
        log: DurableLog<Entry>,
        process_epoch: ProcessEpoch,
    }

    #[effect(family = "storage", operation = "append")]
    async fn append_commit(
        cx: &Cx,
        state: &mut Replica,
        entry: Entry,
    ) -> Outcome<Stable<Entry>, Error> {
        let pending = state.log.reserve(cx, entry).await?;
        let submitted = pending.submit(cx).await?;
        let stable = submitted.sync(cx).await?;
        Outcome::ok(stable)
    }

    #[action]
    async fn coordinate(
        cx: &Cx,
        state: &mut Replica,
        request: Request,
    ) -> Outcome<Response, Error> {
        let prepared = collect_quorum(cx, request.epoch).await?;
        validate_prior_values(&prepared)?;

        let local = append_commit(cx, state, request.entry()).await?;
        let remote = replicate_quorum(cx, request.entry()).await?;
        require_stable_quorum(local, remote)?;

        // The acknowledgement is causally after stable quorum evidence.
        reply(cx, Response::Committed(request)).await
    }

    #[invariant]
    fn no_live_children_after_close(snapshot: RuntimeSnapshot) -> bool {
        snapshot.regions.iter().all(|r| !r.closed || r.live_children == 0)
    }
}

#[refinement(
    source = "replicated_register::RuntimeView",
    target = "abstract_register::State",
    observers = ["client"]
)]
fn runtime_to_abstract(c: &RuntimeConfiguration) -> AbstractRelation {
    relate! {
        // Only entries backed by stable quorum evidence become chosen.
        abstract.chosen ==
            stable_quorum_entries(c)
                .group_by_epoch()
                .require_unique_value()
    }
}

#[observer(name = "client")]
fn client_observation(e: &CirEvent) -> Option<ClientEvent> {
    match e.effect_transition() {
        ("reply", "Reserved", "Committed") => Some(ClientEvent::Response(e.payload())),
        _ => None,
    }
}
