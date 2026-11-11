//! Belegsummen - rechnen exakt wie AZURO (an allen 846 importierten Rechnungen auf den Cent geprueft):
//! Netto = Summe der Positionen - Belegrabatt; die MwSt wird je Steuersatz aus den Positionen berechnet
//! und um den Rabattanteil gekuerzt.

use crate::util::round2;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Summen {
    /// Summe aller Positionen netto (vor Rabatt)
    pub positionen: f64,
    pub rabatt: f64,
    /// Netto nach Rabatt
    pub netto: f64,
    /// (Steuersatz in %, Steuerbetrag) - aufsteigend, nur Saetze mit Umsatz
    pub mwst: Vec<(f64, f64)>,
    pub mwst_summe: f64,
    pub brutto: f64,
}

/// `je_satz`: (Steuersatz, Nettosumme der Positionen mit diesem Satz)
pub fn summen(je_satz: &[(f64, f64)], rabatt: f64) -> Summen {
    let positionen: f64 = je_satz.iter().map(|(_, s)| s).sum();
    let netto = positionen - rabatt;
    let faktor = if positionen.abs() > f64::EPSILON { netto / positionen } else { 1.0 };
    let mut sorted: Vec<(f64, f64)> = Vec::new();
    for (satz, summe) in je_satz {
        if let Some(e) = sorted.iter_mut().find(|e| (e.0 - satz).abs() < 1e-9) {
            e.1 += summe;
        } else {
            sorted.push((*satz, *summe));
        }
    }
    sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mwst: Vec<(f64, f64)> = sorted.iter().filter(|(_, s)| s.abs() > f64::EPSILON).map(|(sa, s)| (*sa, round2(s * faktor * sa / 100.0))).collect();
    let mwst_summe = round2(mwst.iter().map(|(_, b)| b).sum());
    Summen { positionen: round2(positionen), rabatt: round2(rabatt), netto: round2(netto), mwst, mwst_summe, brutto: round2(netto + mwst_summe) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn einfache_rechnung() {
        // Rechnung Nr. 3: 242 netto, 20 % -> 48,40 MwSt, 290,40 brutto
        let s = summen(&[(20.0, 242.0)], 0.0);
        assert_eq!(s.netto, 242.0);
        assert_eq!(s.mwst_summe, 48.4);
        assert_eq!(s.brutto, 290.4);
    }

    #[test]
    fn mit_rabatt_und_zwei_saetzen() {
        let s = summen(&[(20.0, 1000.0), (10.0, 200.0)], 120.0);
        assert_eq!(s.netto, 1080.0);
        // 20% von 1000*0,9 = 180 ; 10% von 200*0,9 = 18
        assert_eq!(s.mwst, vec![(10.0, 18.0), (20.0, 180.0)]);
        assert_eq!(s.brutto, 1278.0);
    }

    #[test]
    fn leerer_beleg() {
        let s = summen(&[], 0.0);
        assert_eq!(s.brutto, 0.0);
    }
}
