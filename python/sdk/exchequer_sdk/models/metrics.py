from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, cast

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

T = TypeVar("T", bound="Metrics")


@_attrs_define
class Metrics:
    """The usual measures, as fractions (null without a denominator). A row
    not guessed counts against recall and accuracy, not precision; each row
    has one true value, so accuracy is the micro recall. Macro means are
    over the classes (values, or accounts) either true or guessed, a class
    never guessed or never true counting 0.

        Attributes:
            samples (int): Rows (or statements).
            accuracy (float | None | Unset):
            coverage (float | None | Unset): Guessed, of the samples.
            f1 (float | None | Unset):
            macro_f1 (float | None | Unset):
            macro_precision (float | None | Unset):
            macro_recall (float | None | Unset):
            precision (float | None | Unset): Right, of the guesses.
            recall (float | None | Unset): Right, of the samples.
    """

    samples: int
    accuracy: float | None | Unset = UNSET
    coverage: float | None | Unset = UNSET
    f1: float | None | Unset = UNSET
    macro_f1: float | None | Unset = UNSET
    macro_precision: float | None | Unset = UNSET
    macro_recall: float | None | Unset = UNSET
    precision: float | None | Unset = UNSET
    recall: float | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)

    def to_dict(self) -> dict[str, Any]:
        samples = self.samples

        accuracy: float | None | Unset
        if isinstance(self.accuracy, Unset):
            accuracy = UNSET
        else:
            accuracy = self.accuracy

        coverage: float | None | Unset
        if isinstance(self.coverage, Unset):
            coverage = UNSET
        else:
            coverage = self.coverage

        f1: float | None | Unset
        if isinstance(self.f1, Unset):
            f1 = UNSET
        else:
            f1 = self.f1

        macro_f1: float | None | Unset
        if isinstance(self.macro_f1, Unset):
            macro_f1 = UNSET
        else:
            macro_f1 = self.macro_f1

        macro_precision: float | None | Unset
        if isinstance(self.macro_precision, Unset):
            macro_precision = UNSET
        else:
            macro_precision = self.macro_precision

        macro_recall: float | None | Unset
        if isinstance(self.macro_recall, Unset):
            macro_recall = UNSET
        else:
            macro_recall = self.macro_recall

        precision: float | None | Unset
        if isinstance(self.precision, Unset):
            precision = UNSET
        else:
            precision = self.precision

        recall: float | None | Unset
        if isinstance(self.recall, Unset):
            recall = UNSET
        else:
            recall = self.recall

        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update(
            {
                "samples": samples,
            }
        )
        if accuracy is not UNSET:
            field_dict["accuracy"] = accuracy
        if coverage is not UNSET:
            field_dict["coverage"] = coverage
        if f1 is not UNSET:
            field_dict["f1"] = f1
        if macro_f1 is not UNSET:
            field_dict["macro_f1"] = macro_f1
        if macro_precision is not UNSET:
            field_dict["macro_precision"] = macro_precision
        if macro_recall is not UNSET:
            field_dict["macro_recall"] = macro_recall
        if precision is not UNSET:
            field_dict["precision"] = precision
        if recall is not UNSET:
            field_dict["recall"] = recall

        return field_dict

    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        samples = d.pop("samples")

        def _parse_accuracy(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        accuracy = _parse_accuracy(d.pop("accuracy", UNSET))

        def _parse_coverage(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        coverage = _parse_coverage(d.pop("coverage", UNSET))

        def _parse_f1(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        f1 = _parse_f1(d.pop("f1", UNSET))

        def _parse_macro_f1(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        macro_f1 = _parse_macro_f1(d.pop("macro_f1", UNSET))

        def _parse_macro_precision(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        macro_precision = _parse_macro_precision(d.pop("macro_precision", UNSET))

        def _parse_macro_recall(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        macro_recall = _parse_macro_recall(d.pop("macro_recall", UNSET))

        def _parse_precision(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        precision = _parse_precision(d.pop("precision", UNSET))

        def _parse_recall(data: object) -> float | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(float | None | Unset, data)

        recall = _parse_recall(d.pop("recall", UNSET))

        metrics = cls(
            samples=samples,
            accuracy=accuracy,
            coverage=coverage,
            f1=f1,
            macro_f1=macro_f1,
            macro_precision=macro_precision,
            macro_recall=macro_recall,
            precision=precision,
            recall=recall,
        )

        metrics.additional_properties = d
        return metrics

    @property
    def additional_keys(self) -> list[str]:
        return list(self.additional_properties.keys())

    def __getitem__(self, key: str) -> Any:
        return self.additional_properties[key]

    def __setitem__(self, key: str, value: Any) -> None:
        self.additional_properties[key] = value

    def __delitem__(self, key: str) -> None:
        del self.additional_properties[key]

    def __contains__(self, key: str) -> bool:
        return key in self.additional_properties
